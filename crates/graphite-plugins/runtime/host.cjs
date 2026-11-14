'use strict';
// One trusted plugin per process. stdout belongs exclusively to the framed RPC.
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { AsyncLocalStorage } = require('node:async_hooks');
const { format } = require('node:util');
const output = process.stdout.write.bind(process.stdout);
for (const level of ['log', 'info', 'debug', 'warn', 'error']) {
  console[level] = (...args) => process.stderr.write(format(...args) + '\n');
}
process.stdout.write = process.stderr.write.bind(process.stderr);
const context = new AsyncLocalStorage();
const root = path.resolve(process.argv[2]);
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'graphite-plugin.json'), 'utf8'));
const handlers = new Map(), listeners = new Map(), saveParticipants = new Map(), pending = new Map(), incoming = new Map();
const treeProviders = new Map(), webviewPanels = new Map();
const subscriptions = [];
let session, next = 0, buffer = Buffer.alloc(0), activated = false, plugin, shuttingDown = false;
const MAX_FRAME = 16 * 1024 * 1024, MAX_PENDING = 128;
function send(message) {
  const bytes = Buffer.from(JSON.stringify(message));
  if (bytes.length > MAX_FRAME) throw new Error('RPC frame too large');
  const size = Buffer.alloc(4); size.writeUInt32BE(bytes.length);
  output(Buffer.concat([size, bytes]));
}
function error(kind, message) { return { kind, message }; }
function request(request) {
  if (shuttingDown) return Promise.reject(new Error('Plugin is shutting down'));
  if (pending.size >= MAX_PENDING) return Promise.reject(new Error('Too many pending requests'));
  const id = ++next;
  return new Promise((resolve, reject) => {
    const expire = () => { pending.delete(id); send({ Cancel: { session, id } }); reject(new Error('RPC deadline exceeded')); };
    const entry = { resolve, reject, expire, timer: setTimeout(expire, 30000) };
    pending.set(id, entry);
    try { send({ Request: { session, id, request } }); }
    catch (e) { clearTimeout(entry.timer); pending.delete(id); reject(e); }
  });
}
const service = (method, params) => request({ Service: { request: params === undefined ? { method } : { method, params }, parent: context.getStore()?.id ?? null } });
function disposable(resource, cleanup) {
  let disposed = false;
  const value = { async dispose() { if (disposed) return; disposed = true; cleanup(); if (!shuttingDown) await request({ Dispose: { resource } }); } };
  subscriptions.push(value);
  return value;
}
let statusCounter = 0, webviewCounter = 0;
const storageArea = scope => ({
  get: key => service('storageGet', key === undefined ? { scope } : { scope, key }),
  set: (key, value) => service('storageSet', { scope, key, value }),
  delete: key => service('storageSet', { scope, key }),
});
const graphite = {
  commands: {
    list: () => service('commands'),
    execute: (id, arguments_ = null, target = { kind: 'global' }) => service('execute', { id, arguments: arguments_, target }),
    async registerCommand(id, handler) {
      const declared = manifest.commands?.find(c => c.id === id);
      if (!declared || typeof handler !== 'function' || handlers.has(id)) throw new Error('Command must be declared and unique');
      const result = await request({ RegisterCommand: { id, title: declared.title, category: manifest.id, targets: ['Global', 'View', 'Widget'], enablement: 'true' } });
      handlers.set(id, handler);
      return disposable(result.resource, () => handlers.delete(id));
    },
  },
  documents: {
    list: () => service('documents'),
    open: uri => service('open', { uri }),
    save: (uri, reason = 'manual') => service('save', { uri, reason }),
    snapshot: uri => service('snapshot', { uri }),
    applyEdits: edits => service('applyEdits', { edits }),
    async onDidChange(listener) {
      const { resource } = await request('SubscribeDocuments');
      listeners.set(resource, listener);
      return disposable(resource, () => listeners.delete(resource));
    },
    async onWillSave(metadata, participant) {
      if (!metadata?.id || typeof participant !== 'function') throw new Error('Invalid save participant');
      const { resource } = await request({ RegisterSaveParticipant: { id: metadata.id, languages: metadata.languages ?? [] } });
      saveParticipants.set(resource, participant);
      return disposable(resource, () => saveParticipants.delete(resource));
    },
  },
  workspace: { get: () => service('workspace'), configuration: () => service('configuration') },
  window: {
    showMessage: text => service('message', { text }),
    focusView: view => service('focus', { view }),
    showInputBox: (options = {}) => service('promptInput', options),
    showQuickPick: (items = [], options = {}) => {
      const normalized = items.map(item => typeof item === 'string' ? { label: item } : item);
      return service('quickPick', { items: normalized, ...options });
    },
    createStatusBarItem: (id = `status-${++statusCounter}`) => {
      let disposed = false;
      const item = {
        text: '',
        command: undefined,
        async show() {
          if (disposed) return;
          await service('setStatusBarItem', { id, text: item.text, ...(item.command ? { command: item.command } : {}) });
        },
        async hide() {
          if (disposed) return;
          await service('disposeStatusBarItem', { id });
        },
        async dispose() {
          if (disposed) return;
          disposed = true;
          if (!shuttingDown) await service('disposeStatusBarItem', { id });
        },
      };
      subscriptions.push(item);
      return item;
    },
    async createOutputChannel(name) {
      const { channel } = await service('createOutputChannel', { name });
      let disposed = false;
      const out = {
        append: text => disposed ? Promise.resolve() : service('appendOutput', { channel, text: String(text), replace: false }),
        appendLine: text => disposed ? Promise.resolve() : service('appendOutput', { channel, text: String(text) + '\n', replace: false }),
        clear: () => disposed ? Promise.resolve() : service('appendOutput', { channel, text: '', replace: true }),
        show: () => disposed ? Promise.resolve() : service('showOutput', { channel }),
        async dispose() { disposed = true; },
      };
      subscriptions.push(out);
      return out;
    },
    async registerTreeDataProvider(viewId, provider) {
      if (typeof viewId !== 'string' || typeof provider?.getChildren !== 'function') {
        throw new Error('Invalid TreeDataProvider');
      }
      const entry = { provider, nodes: new Map(), changeSub: undefined };
      treeProviders.set(viewId, entry);
      if (typeof provider.onDidChangeTreeData === 'function') {
        entry.changeSub = provider.onDidChangeTreeData(element => {
          const elementId = typeof element === 'string' ? element : element?.id ?? null;
          if (!shuttingDown) service('refreshTreeView', { viewId, elementId }).catch(() => {});
        });
      }
      await service('registerTreeDataProvider', { viewId });
      const handle = {
        async refresh(element) {
          const elementId = typeof element === 'string' ? element : element?.id ?? null;
          await service('refreshTreeView', { viewId, elementId });
        },
        async dispose() {
          treeProviders.delete(viewId);
          await entry.changeSub?.dispose?.();
        },
      };
      subscriptions.push(handle);
      return handle;
    },
    async createWebviewPanel(viewType, title, options = {}) {
      const handle = `webview-${++webviewCounter}`;
      let currentTitle = String(title ?? '');
      let currentHtml = String(options?.html ?? '');
      let disposed = false;
      const messageListeners = new Set();
      const disposeListeners = new Set();
      const panelState = { messageListeners, disposeListeners };
      webviewPanels.set(handle, panelState);
      await service('createWebviewPanel', {
        handle,
        viewType: String(viewType ?? 'graphite.webview'),
        title: currentTitle,
        html: currentHtml,
      });
      const webview = {
        get html() { return currentHtml; },
        set html(value) {
          currentHtml = String(value ?? '');
          if (!disposed && !shuttingDown) service('setWebviewHtml', { handle, html: currentHtml }).catch(() => {});
        },
        async setHtml(value) {
          currentHtml = String(value ?? '');
          if (!disposed) await service('setWebviewHtml', { handle, html: currentHtml });
        },
        async postMessage(message) {
          if (!disposed) await service('postWebviewMessage', { handle, message: message ?? null });
        },
        onDidReceiveMessage(listener) {
          messageListeners.add(listener);
          return { async dispose() { messageListeners.delete(listener); } };
        },
      };
      const panel = {
        viewType: String(viewType ?? 'graphite.webview'),
        get title() { return currentTitle; },
        set title(value) {
          currentTitle = String(value ?? '');
          if (!disposed && !shuttingDown) service('setWebviewTitle', { handle, title: currentTitle }).catch(() => {});
        },
        async setTitle(value) {
          currentTitle = String(value ?? '');
          if (!disposed) await service('setWebviewTitle', { handle, title: currentTitle });
        },
        webview,
        onDidDispose(listener) {
          disposeListeners.add(listener);
          return { async dispose() { disposeListeners.delete(listener); } };
        },
        async dispose() {
          if (disposed) return;
          disposed = true;
          webviewPanels.delete(handle);
          for (const listener of disposeListeners) await listener();
          if (!shuttingDown) await service('disposeWebviewPanel', { handle });
        },
      };
      subscriptions.push(panel);
      return panel;
    },
  },
  storage: {
    global: storageArea('global'),
    workspace: storageArea('workspace'),
  },
};
async function shutdown() {
  if (shuttingDown) return;
  shuttingDown = true;
  const deadline = setTimeout(() => process.exit(0), 1000);
  try { await plugin?.deactivate?.(); await Promise.allSettled(subscriptions.map(d => d.dispose())); }
  finally { clearTimeout(deadline); process.exit(0); }
}
async function receive(message) {
  const kind = typeof message === 'string' ? message : Object.keys(message)[0];
  const data = message[kind];
  if (kind === 'Hello') {
    if (session !== undefined || data.version !== 2) throw new Error('Incompatible handshake');
    session = data.session;
    send({ Ready: { version: 2, session } });
    plugin = await import(pathToFileURL(path.resolve(root, manifest.main)).href);
    const api = plugin.activate ? plugin : plugin.default;
    if (typeof api?.activate !== 'function') throw new Error('Plugin must export activate');
    plugin = api;
    await plugin.activate({ subscriptions }, graphite);
    activated = true;
    await request('Activated');
    return;
  }
  if (data?.session !== session) return;
  if (kind === 'Response') {
    const entry = pending.get(data.id); if (!entry) return;
    clearTimeout(entry.timer); pending.delete(data.id);
    if (Object.hasOwn(data.result, 'Ok')) entry.resolve(data.result.Ok);
    else entry.reject(Object.assign(new Error(JSON.stringify(data.result.Err)), { detail: data.result.Err }));
  } else if (kind === 'WaitingForUser') {
    const entry = pending.get(data.id); if (entry) clearTimeout(entry.timer);
  } else if (kind === 'Resumed') {
    const entry = pending.get(data.id); if (entry) entry.timer = setTimeout(entry.expire, 30000);
  } else if (kind === 'Event') {
    // Subscription replies and their first events may share one transport chunk.
    await Promise.resolve();
    await listeners.get(data.resource)?.(data.event);
  } else if (kind === 'Cancel') {
    incoming.get(data.id)?.abort();
  } else if (kind === 'Shutdown') {
    await shutdown();
  } else if (kind === 'Request') {
    if (incoming.size >= MAX_PENDING || incoming.has(data.id)) throw new Error('Invalid incoming request');
    const abort = new AbortController(); incoming.set(data.id, abort);
    let result;
    try {
      if (!activated) throw new Error('Plugin is not active');
      let value;
      if (data.request.Invoke) {
        const invoke = data.request.Invoke;
        if (!handlers.has(invoke.command)) throw new Error('Plugin command is unavailable');
        value = await context.run({ id: data.id }, () => handlers.get(invoke.command)(invoke.arguments, abort.signal));
      } else if (data.request.ParticipateInSave) {
        const { resource, request: saveRequest } = data.request.ParticipateInSave;
        if (!saveParticipants.has(resource)) throw new Error('Save participant is unavailable');
        const edits = await context.run({ id: data.id }, () => saveParticipants.get(resource)(saveRequest, abort.signal));
        value = { invocation: saveRequest.invocation, version: saveRequest.document.version, edits: Array.isArray(edits) ? edits : [] };
      } else if (data.request.ProvideTreeChildren) {
        const { view_id: viewId, element_id: elementId } = data.request.ProvideTreeChildren;
        const entry = treeProviders.get(viewId);
        if (!entry) throw new Error(`TreeDataProvider is unavailable for ${viewId}`);
        const element = elementId == null ? undefined : (entry.nodes.get(elementId) ?? { id: elementId, label: elementId });
        const raw = (await context.run({ id: data.id }, () => entry.provider.getChildren(element, abort.signal))) ?? [];
        const items = [];
        for (let i = 0; i < raw.length; i++) {
          const candidate = raw[i];
          const item = entry.provider.getTreeItem ? await entry.provider.getTreeItem(candidate) : candidate;
          const itemId = String(item?.id ?? `${elementId ?? 'root'}:${i}:${item?.label ?? ''}`);
          entry.nodes.set(itemId, candidate);
          items.push({
            id: itemId,
            label: String(item?.label ?? itemId),
            ...(item?.description != null ? { description: String(item.description) } : {}),
            ...(item?.tooltip != null ? { tooltip: String(item.tooltip) } : {}),
            collapsibleState: item?.collapsibleState ?? 'none',
            ...(item?.command ? {
              command: {
                id: String(item.command.id ?? item.command.command),
                ...(item.command.title != null ? { title: String(item.command.title) } : {}),
                arguments: Array.isArray(item.command.arguments) ? item.command.arguments : [],
              },
            } : {}),
            ...(item?.contextValue != null ? { contextValue: String(item.contextValue) } : {}),
          });
        }
        value = items;
      } else if (data.request.DeliverWebviewMessage) {
        const { handle, message: payload } = data.request.DeliverWebviewMessage;
        const panelState = webviewPanels.get(handle);
        if (panelState) {
          for (const listener of panelState.messageListeners) {
            await context.run({ id: data.id }, () => listener(payload));
          }
        }
        value = null;
      } else {
        throw new Error('Unsupported host request');
      }
      result = abort.signal.aborted ? { Err: { kind: 'cancelled' } } : { Ok: value ?? null };
    } catch (e) { result = { Err: error('failed', String(e)) }; }
    incoming.delete(data.id);
    send({ Response: { session, id: data.id, result } });
  }
}
process.stdin.on('data', chunk => {
  buffer = Buffer.concat([buffer, chunk]);
  while (buffer.length >= 4) {
    const size = buffer.readUInt32BE(0);
    if (!size || size > MAX_FRAME) { console.error('Invalid RPC frame'); process.exit(1); }
    if (buffer.length < size + 4) break;
    let message;
    try { message = JSON.parse(buffer.subarray(4, size + 4)); }
    catch (e) { console.error(e); process.exit(1); }
    buffer = buffer.subarray(size + 4);
    receive(message).catch(e => { console.error(e); process.exit(1); });
  }
});
process.stdin.on('end', () => { shutdown().catch(() => process.exit(0)); });
process.on('uncaughtException', e => { console.error(e); process.exit(1); });
process.on('unhandledRejection', e => { console.error(e); process.exit(1); });
