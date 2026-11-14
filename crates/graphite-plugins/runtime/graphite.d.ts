export interface Disposable { dispose(): Promise<void> }
export interface Position { line: number; character: number }
export interface TextEdit { start: Position; end: Position; text: string }
export interface DocumentSnapshot {
  id: number | null; uri: string; text: string; version: number | null;
  language: string | null; dirty: boolean; read_only: boolean;
}
export interface DocumentEdit { uri: string; version: number | null; edits: TextEdit[] }
export interface DocumentEvent { kind: 'opened' | 'changed' | 'closed'; document: DocumentSnapshot }
export type CommandTargetKind = 'Global' | 'View' | 'Widget';
export type CommandTarget =
  | { kind: 'global' }
  | { kind: 'view'; id: number }
  | { kind: 'widget'; id: number };
export interface CommandMetadata {
  id: string; title: string; category: string;
  targets: CommandTargetKind[]; enablement: string;
}
export type SaveReason = 'manual' | 'saveAll' | 'close' | 'workspace';
export interface SaveOutcome {
  document: number; version: number; dirty: boolean; warnings: string[];
}
export interface SaveParticipantMetadata { id: string; languages?: string[] }
export interface SaveParticipantRequest {
  invocation: number; reason: SaveReason; document: DocumentSnapshot;
}
export interface InputBoxOptions {
  prompt?: string; placeHolder?: string; value?: string; password?: boolean;
}
export interface QuickPickItem {
  label: string; id?: string; description?: string;
}
export interface QuickPickOptions {
  placeHolder?: string; canPickMany?: boolean;
}
export interface StatusBarItem extends Disposable {
  text: string;
  command?: string;
  show(): Promise<void>;
  hide(): Promise<void>;
}
export interface OutputChannel extends Disposable {
  append(text: string): Promise<void>;
  appendLine(text: string): Promise<void>;
  clear(): Promise<void>;
  show(): Promise<void>;
}
export type TreeCollapsibleState = 'none' | 'collapsed' | 'expanded';
export interface TreeItemCommand {
  id: string;
  title?: string;
  arguments?: unknown[];
}
export interface TreeItem {
  id?: string;
  label: string;
  description?: string;
  tooltip?: string;
  collapsibleState?: TreeCollapsibleState;
  command?: TreeItemCommand;
  contextValue?: string;
}
export interface TreeDataProvider<T = TreeItem> {
  getChildren(element?: T, signal?: AbortSignal): Promise<T[] | undefined> | T[] | undefined;
  getTreeItem?(element: T): Promise<TreeItem> | TreeItem;
  onDidChangeTreeData?(listener: (element?: T) => void): Disposable | void;
}
export interface TreeViewHandle<T = TreeItem> extends Disposable {
  refresh(element?: T | string): Promise<void>;
}
export interface WebviewPanelOptions {
  html?: string;
}
export interface WebviewHandle {
  html: string;
  setHtml(html: string): Promise<void>;
  postMessage(message: unknown): Promise<void>;
  onDidReceiveMessage(listener: (message: unknown) => void | Promise<void>): Disposable;
}
export interface WebviewPanel extends Disposable {
  readonly viewType: string;
  title: string;
  readonly webview: WebviewHandle;
  setTitle(title: string): Promise<void>;
  onDidDispose(listener: () => void | Promise<void>): Disposable;
}
export interface StorageArea {
  get<T = unknown>(key?: string): Promise<T | null>;
  set(key: string, value: unknown): Promise<void>;
  delete(key: string): Promise<void>;
}
export interface Context { subscriptions: Disposable[] }
export interface Graphite {
  commands: {
    list(): Promise<CommandMetadata[]>;
    execute(id: string, arguments_?: unknown, target?: CommandTarget): Promise<unknown>;
    registerCommand(id: string, handler: (arguments_: unknown, signal: AbortSignal) => unknown | Promise<unknown>): Promise<Disposable>;
  };
  documents: {
    list(): Promise<DocumentSnapshot[]>;
    open(uri: string): Promise<DocumentSnapshot>;
    save(uri: string, reason?: SaveReason): Promise<SaveOutcome>;
    snapshot(uri: string): Promise<DocumentSnapshot>;
    applyEdits(edits: DocumentEdit[]): Promise<void>;
    onDidChange(listener: (event: DocumentEvent) => void | Promise<void>): Promise<Disposable>;
    onWillSave(
      metadata: SaveParticipantMetadata,
      participant: (request: SaveParticipantRequest, signal: AbortSignal) => TextEdit[] | void | Promise<TextEdit[] | void>,
    ): Promise<Disposable>;
  };
  workspace: { get(): Promise<{ root: string }>; configuration(): Promise<unknown> };
  window: {
    showMessage(text: string): Promise<void>;
    focusView(view: number): Promise<void>;
    showInputBox(options?: InputBoxOptions): Promise<string | null>;
    showQuickPick(items: QuickPickItem[] | string[], options?: QuickPickOptions): Promise<QuickPickItem | QuickPickItem[] | null>;
    createStatusBarItem(id?: string): StatusBarItem;
    createOutputChannel(name: string): Promise<OutputChannel>;
    registerTreeDataProvider<T = TreeItem>(viewId: string, provider: TreeDataProvider<T>): Promise<TreeViewHandle<T>>;
    createWebviewPanel(viewType: string, title: string, options?: WebviewPanelOptions): Promise<WebviewPanel>;
  };
  storage: {
    global: StorageArea;
    workspace: StorageArea;
  };
}
