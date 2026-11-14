// Adapt the pinned Code OSS wire protocol to Graphite's private child-process
// channel. Graphite owns all workbench operations and authorization in Rust.
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, realpath, rm } from 'node:fs/promises';
import { createRequire } from 'node:module';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';
import { VSBuffer } from './code/out/vs/base/common/buffer.js';
import { CancellationToken, CancellationTokenSource } from './code/out/vs/base/common/cancellation.js';
import { URI } from './code/out/vs/base/common/uri.js';
import { PersistentProtocol } from './code/out/vs/base/parts/ipc/common/ipc.net.js';
import { NodeSocket } from './code/out/vs/base/parts/ipc/node/ipc.net.js';
import { ExtHostContext, MainContext } from './code/out/vs/workbench/api/common/extHost.protocol.js';
import { RPCProtocol } from './code/out/vs/workbench/services/extensions/common/rpcProtocol.js';
import { isEngineValid } from './code/out/vs/platform/extensions/common/extensionValidator.js';

const version = '1.145.2';
const commit = 'c4453f8ea430795f941c1b2ff6c9321135a74a02';
const codeRoot = fileURLToPath(new URL('./code/', import.meta.url));
const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
const pending = new Map();
const calls = new Map();
let nextId = 0;
let child, socket, server, socketDirectory, rpc;
let stopping = false;
let synchronization = Promise.resolve();
const documents = new Map();
let activeEditor = null;
// Debug adapter factories extensions registered, by debug type, so Graphite can ask for one.
const debugFactories = new Map();
// Debug configuration providers by handle: { type, hasResolve, hasResolve2 }.
const debugConfigProviders = new Map();
let workspaceRootUri;

function send(message) {
    const json = JSON.stringify(message, (_, value) => value?.$mid === 1
        ? URI.revive(value).toString().replace(/^file:\/\/\/([a-z])%3A(?=\/)/i, 'file:///$1:') : value);
    process.stdout.write(json + '\n');
}

async function stop(error) {
    if (stopping) return;
    stopping = true;
    if (error) send({ type: 'failure', error: String(error) });
    socket?.destroy();
    server?.close();
    child?.kill();
    if (socketDirectory) await rm(socketDirectory, { recursive: true, force: true });
    process.exitCode = error ? 1 : 0;
    input.close();
    process.stdin.destroy();
}

function request(actor, method, args) {
    if (pending.size >= 128) throw new Error('Too many pending Graphite requests');
    const id = ++nextId;
    return new Promise((resolve, reject) => {
        const token = args.at(-1);
        if (token?.isCancellationRequested) return resolve(undefined);
        const cancellation = token?.onCancellationRequested?.(() => {
            pending.delete(id);
            send({ type: 'cancel', id });
            resolve(undefined);
        });
        pending.set(id, { resolve, reject, method, cancellation });
        send({ type: 'request', id, actor, method, args: cancellation ? args.slice(0, -1) : args });
    });
}

async function extensions(packages) {
    const allExtensions = [];
    const activationEvents = Object.create(null);
    for (const { directory, id, version: expected } of packages) {
        const manifest = JSON.parse(await readFile(path.join(directory, 'package.json'), 'utf8'));
        if (`${manifest.publisher}.${manifest.name}`.toLowerCase() !== id || manifest.version !== expected) {
            throw new Error(`Installed VSIX identity changed: ${id}`);
        }
        if (typeof manifest.engines?.vscode !== 'string' || !isEngineValid(manifest.engines.vscode, version)) {
            continue;
        }
        try {
            const location = await realpath(directory);
            const require = createRequire(path.join(location, 'package.json'));
            const entry = await realpath(require.resolve(path.resolve(location, manifest.main)));
            const relative = path.relative(location, entry);
            if (!relative || relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) continue;
        } catch {
            continue;
        }
        const identifier = { value: id, _lower: id };
        const events = new Set(manifest.activationEvents ?? []);
        for (const command of manifest.contributes?.commands ?? []) {
            if (typeof command.command === 'string') events.add(`onCommand:${command.command}`);
        }
        allExtensions.push({ ...manifest, identifier, extensionLocation: URI.file(directory),
            isBuiltin: false, isUserBuiltin: false, isUnderDevelopment: false, targetPlatform: 'universal' });
        activationEvents[id] = [...events];
    }
    return { versionId: 1, allExtensions, activationEvents,
        myExtensions: allExtensions.map(extension => extension.identifier) };
}

async function start({ packages, root, rootUri, storage, locale, configuration }) {
    workspaceRootUri = rootUri;
    const snapshot = await extensions(packages);
    if (stopping) return;
    const declared = new Map();
    for (const extension of snapshot.allExtensions) {
        for (const command of extension.contributes?.commands ?? []) {
            const id = command.command;
            declared.set(id, declared.has(id) ? null : extension.identifier);
        }
    }
    const logs = path.join(storage, 'logs');
    for (const directory of [logs, path.join(storage, 'global'), path.join(storage, 'workspaces')]) {
        await mkdir(directory, { recursive: true });
    }
    if (stopping) return;
    const workspace = {
        id: createHash('sha256').update(root).digest('hex'),
        name: path.basename(root) || root,
        folders: [{ uri: URI.parse(rootUri), name: path.basename(root) || root, index: 0 }]
    };
    let address;
    if (process.platform === 'win32') {
        address = '\\\\.\\pipe\\graphite-code-oss-' + randomUUID();
    } else {
        socketDirectory = await mkdtemp(path.join(os.tmpdir(), 'graphite-'));
        address = path.join(socketDirectory, 'host');
    }
    server = net.createServer();
    server.listen(address);
    await once(server, 'listening');
    if (stopping) return;
    const connection = once(server, 'connection');
    child = spawn(process.execPath, [path.join(codeRoot, 'out/bootstrap-fork.js')], {
        cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'inherit'],
        env: { ...process.env,
            VSCODE_ESM_ENTRYPOINT: 'vs/workbench/api/node/extensionHostProcess',
            VSCODE_EXTHOST_IPC_HOOK: address,
            VSCODE_HANDLES_UNCAUGHT_ERRORS: '1',
            VSCODE_NLS_CONFIG: JSON.stringify({ userLocale: locale, osLocale: locale,
                resolvedLanguage: 'en',
                locale, availableLanguages: {} }) },
    });
    child.stdout.pipe(process.stderr);
    child.on('error', stop);
    child.on('exit', (code, signal) => stop(new Error(`Code OSS exited (${signal ?? code})`)));
    [socket] = await connection;
    server.close();
    const protocol = new PersistentProtocol({ socket: new NodeSocket(socket, 'graphite') });
    const init = {
        commit, version, parentPid: process.pid, workspace, extensions: snapshot,
        environment: {
            appName: 'Graphite', appHost: 'desktop', appRoot: URI.file(codeRoot),
            appLanguage: locale, appUriScheme: 'graphite', isExtensionDevelopmentDebug: false,
            isExtensionTelemetryLoggingOnly: true, globalStorageHome: URI.file(path.join(storage, 'global')),
            workspaceStorageHome: URI.file(path.join(storage, 'workspaces'))
        },
        telemetryInfo: { sessionId: '', machineId: '', sqmId: '', devDeviceId: '', firstSessionDate: '' },
        logLevel: 2, loggers: [], logsLocation: URI.file(logs), autoStart: true,
        remote: { isRemote: false, connectionData: null },
        consoleForward: { includeStack: false, logNative: true }, uiKind: 1
    };
    await new Promise(resolve => {
        let ready = false;
        const listener = protocol.onMessage(message => {
            if (!ready && message.byteLength === 1 && message.readUInt8(0) === 2) {
                ready = true;
                protocol.send(VSBuffer.fromString(JSON.stringify(init)));
            } else if (ready && message.byteLength === 1 && message.readUInt8(0) === 1) {
                listener.dispose();
                rpc = new RPCProtocol(protocol);
                for (const [name, identifier] of Object.entries(MainContext)) {
                    rpc.set(identifier, new Proxy(Object.create(null), {
                        get: (_, method) => typeof method === 'string' && method.startsWith('$')
                            ? (...args) => {
                                if (name === 'MainThreadDebugService') trackDebugFactory(method, args);
                                if (name === 'MainThreadCommands' && method === '$executeCommand' && declared.has(args[0])) {
                                    const owner = declared.get(args[0]);
                                    if (!owner) throw new Error(`Multiple VSIX extensions declare ${args[0]}`);
                                    return executeCommand(owner, args[0], args[1]?.value ?? args[1] ?? []);
                                }
                                return request(name, method, args);
                            }
                            : undefined,
                    }));
                }
                resolve();
            } else {
                void stop(new Error('Unexpected Code OSS handshake message'));
            }
        });
    });
    await Promise.all([
        rpc.getProxy(ExtHostContext.ExtHostWorkspace).$initializeWorkspace(workspace, true),
        rpc.getProxy(ExtHostContext.ExtHostConfiguration).$initializeConfiguration(configurationData(configuration)),
    ]);
    send({ type: 'ready' });
}

function configurationModel(settings) {
    const model = { contents: Object.create(null), keys: [], overrides: [] };
    for (const [key, value] of Object.entries(settings ?? {})) {
        const segments = key.split('.');
        if (segments.some(segment => ['__proto__', 'prototype', 'constructor'].includes(segment))) continue;
        let target = model.contents;
        for (const segment of segments.slice(0, -1)) {
            if (typeof target[segment] !== 'object' || !target[segment]) target[segment] = {};
            target = target[segment];
        }
        target[segments.at(-1)] = value;
        model.keys.push(key);
    }
    return model;
}

function configurationData(configuration) {
    const empty = configurationModel();
    return {
        defaults: empty, policy: empty, application: empty,
        userLocal: configurationModel({ ...configuration?.base, ...configuration?.profile }),
        userRemote: empty, workspace: configurationModel(configuration?.workspace),
        folders: [], configurationScopes: [],
    };
}

async function sync({ documents: snapshots, active, tabSize, insertSpaces }) {
    const incoming = new Map(snapshots.map(document => [document.uri, document]));
    const delta = { addedDocuments: [], removedDocuments: [], addedEditors: [], removedEditors: [] };
    const documentProxy = rpc.getProxy(ExtHostContext.ExtHostDocuments);
    const options = { tabSize, indentSize: tabSize, insertSpaces, cursorStyle: 1, lineNumbers: 1 };
    const activeUri = active?.uri ?? null;
    if (activeEditor !== activeUri) {
        if (activeEditor) delta.removedEditors.push(activeEditor);
        if (activeUri) delta.addedEditors.push({
            id: activeUri, documentUri: URI.parse(activeUri), selections: active.selections,
            options, visibleRanges: [], editorPosition: 1,
        });
        delta.newActiveEditor = activeUri;
    }
    for (const uri of documents.keys()) {
        if (!incoming.has(uri)) delta.removedDocuments.push(URI.parse(uri));
    }
    for (const [uri, document] of incoming) {
        const resource = URI.parse(uri);
        const old = documents.get(uri);
        const eol = document.text.includes('\r\n') ? '\r\n' : '\n';
        if (!old) {
            delta.addedDocuments.push({
                uri: resource, versionId: document.version, languageId: document.language ?? 'plaintext',
                lines: document.text.split(/\r?\n/), EOL: eol, isDirty: document.dirty, encoding: 'utf8',
            });
            continue;
        }
        if (old.version !== document.version) {
            const lines = old.text.split(/\r?\n/);
            await documentProxy.$acceptModelChanged(resource, {
                versionId: document.version, eol, isUndoing: false, isRedoing: false,
                changes: [{ range: { startLineNumber: 1, startColumn: 1,
                    endLineNumber: lines.length, endColumn: lines.at(-1).length + 1 },
                    rangeOffset: 0, rangeLength: old.text.length, text: document.text }],
            }, document.dirty);
        }
        if (old.dirty && !document.dirty) await documentProxy.$acceptModelSaved(resource);
        else if (old.dirty !== document.dirty && old.version === document.version) {
            await documentProxy.$acceptDirtyStateChanged(resource, document.dirty);
        }
        if (old.language !== document.language) {
            await documentProxy.$acceptModelLanguageChanged(resource, document.language ?? 'plaintext');
        }
    }
    await rpc.getProxy(ExtHostContext.ExtHostDocumentsAndEditors).$acceptDocumentsAndEditorsDelta(delta);
    if (activeUri && activeEditor === activeUri) {
        await rpc.getProxy(ExtHostContext.ExtHostEditors).$acceptEditorPropertiesChanged(activeUri, {
            options, selections: { selections: active.selections }, visibleRanges: null,
        });
    }
    activeEditor = activeUri;
    documents.clear();
    for (const [uri, document] of incoming) documents.set(uri, document);
}

async function execute({ id, owner, command, args }) {
    try {
        await synchronization;
        const result = await executeCommand(owner ? { value: owner, _lower: owner } : null, command, Array.isArray(args) ? args : []);
        send({ type: 'result', id, result });
    } catch (error) {
        send({ type: 'result', id, error: String(error) });
    }
}

function trackDebugFactory(method, args) {
    if (method === '$registerDebugAdapterDescriptorFactory') {
        debugFactories.set(args[0], args[1]);
    } else if (method === '$unregisterDebugAdapterDescriptorFactory') {
        for (const [type, handle] of debugFactories) {
            if (handle === args[0]) debugFactories.delete(type);
        }
    } else if (method === '$registerDebugConfigurationProvider') {
        // (type, triggerKind, hasProvide, hasResolve, hasResolve2, handle)
        debugConfigProviders.set(args[5], { type: args[0], hasResolve: !!args[3], hasResolve2: !!args[4] });
    } else if (method === '$unregisterDebugConfigurationProvider') {
        debugConfigProviders.delete(args[0]);
    }
}

// Prepares a debug session for Graphite: activates the extension that provides `debugType`, lets
// its configuration providers resolve the configuration, and asks its adapter factory (if any)
// for the adapter descriptor. Replies with `{ configuration, descriptor }`, or `{ cancelled }`
// when a provider cancels the launch (VS Code's `undefined` / `null` results).
async function provideDebugAdapter({ id, owner, debugType, session }) {
    try {
        await synchronization;
        const extensionId = { value: owner, _lower: owner };
        const activated = await rpc.getProxy(ExtHostContext.ExtHostExtensionService)
            .$activate(extensionId, { startup: false, extensionId, activationEvent: `onDebugResolve:${debugType}` });
        if (!activated) {
            throw new Error(`Could not activate ${owner}. Check its entry point, dependencies and supported Code OSS version.`);
        }
        const debug = rpc.getProxy(ExtHostContext.ExtHostDebugService);
        const folderUri = workspaceRootUri ? URI.parse(workspaceRootUri) : undefined;
        let configuration = session.configuration;
        for (const [method, flag] of [['$resolveDebugConfiguration', 'hasResolve'],
                ['$resolveDebugConfigurationWithSubstitutedVariables', 'hasResolve2']]) {
            for (const [handle, provider] of debugConfigProviders) {
                if (provider.type !== debugType || !provider[flag]) continue;
                const resolved = await debug[method](handle, folderUri, configuration, CancellationToken.None);
                if (resolved === undefined || resolved === null) {
                    send({ type: 'result', id, result: { cancelled: true, openLaunchJson: resolved === null } });
                    return;
                }
                configuration = resolved;
            }
        }
        const factory = debugFactories.get(debugType);
        const descriptor = factory === undefined ? null : await debug.$provideDebugAdapter(factory, {
            id: randomUUID(), type: session.type, name: session.name, folderUri, configuration,
        });
        send({ type: 'result', id, result: { configuration, descriptor: descriptor ?? null } });
    } catch (error) {
        send({ type: 'result', id, error: String(error?.message ?? error) });
    }
}

async function executeCommand(extensionId, command, args) {
    if (extensionId) {
        const activated = await rpc.getProxy(ExtHostContext.ExtHostExtensionService)
            .$activate(extensionId, { startup: false, extensionId, activationEvent: `onCommand:${command}` });
        if (!activated) {
            throw new Error(`Could not activate ${extensionId.value}. Check its entry point, dependencies and supported Code OSS version.`);
        }
    }
    return rpc.getProxy(ExtHostContext.ExtHostCommands).$executeContributedCommand(command, ...args);
}

// The 32-bit little-endian words of a VSBuffer (ignoring a trailing partial word).
function words(buffer) {
    const bytes = buffer.buffer;
    const length = bytes.byteLength - (bytes.byteLength % 4);
    const view = new DataView(bytes.buffer, bytes.byteOffset, length);
    const result = new Array(length / 4);
    for (let index = 0; index < result.length; index++) result[index] = view.getUint32(index * 4, true);
    return result;
}

async function receive(message) {
    switch (message.type) {
        case 'start':
            if (server) throw new Error('Code OSS session already initialized');
            await start(message);
            break;
        case 'reply': {
            const waiter = pending.get(message.id);
            if (waiter) {
                pending.delete(message.id);
                waiter.cancellation?.dispose();
                if ('error' in message) waiter.reject(new Error(message.error));
                else if (waiter.method === '$tryOpenDocument') {
                    await synchronization;
                    waiter.resolve(URI.parse(message.result));
                } else if (waiter.method === '$tryShowTextDocument') {
                    await synchronization;
                    waiter.resolve(message.result);
                } else waiter.resolve(message.result);
            }
            break;
        }
        case 'sync':
            synchronization = synchronization.then(() => sync(message));
            await synchronization;
            break;
        case 'configuration':
            await rpc.getProxy(ExtHostContext.ExtHostConfiguration).$acceptConfigurationChanged(
                configurationData(message.configuration), { keys: [message.key], overrides: [] });
            break;
        case 'execute':
            await execute(message);
            break;
        case 'activateByEvent':
            await synchronization;
            await rpc.getProxy(ExtHostContext.ExtHostExtensionService).$activateByEvent(message.event, 1);
            break;
        case 'provideDebugAdapter':
            await provideDebugAdapter(message);
            break;
        case 'call':
            await synchronization;
            const token = message.cancellable ? new CancellationTokenSource() : undefined;
            if (token) calls.set(message.id, token);
            try {
                const actor = rpc.getProxy(ExtHostContext[message.actor]);
                const args = message.args.map(arg => arg?.$graphiteUri ? URI.parse(arg.$graphiteUri) : arg);
                if (token) args.push(token.token);
                const result = await actor[message.method](...args);
                // Binary answers (semantic tokens) reach Graphite as little-endian 32-bit words.
                const reply = result instanceof VSBuffer ? { $u32: words(result) } : result;
                if (message.id != null) send({ type: 'result', id: message.id, result: reply });
            } catch (error) {
                if (message.id != null) send({ type: 'result', id: message.id, error: String(error) });
            } finally {
                calls.delete(message.id);
                token?.dispose();
            }
            break;
        case 'cancelCall':
            calls.get(message.id)?.cancel();
            break;
        default:
            throw new Error('Unexpected Graphite bridge message');
    }
}

input.on('line', line => {
    try {
        void receive(JSON.parse(line)).catch(stop);
    } catch (error) {
        void stop(error);
    }
});
input.on('close', () => void stop());
