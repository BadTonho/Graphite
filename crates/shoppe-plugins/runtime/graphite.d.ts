export interface Disposable { dispose(): Promise<void> }
export interface Position { line: number; character: number }
export interface TextEdit { start: Position; end: Position; text: string }
export interface DocumentSnapshot {
  id: number | null; uri: string; text: string; version: number | null;
  language: string | null; dirty: boolean; read_only: boolean;
}
export interface DocumentEdit { uri: string; version: number | null; edits: TextEdit[] }
export interface DocumentEvent { kind: 'opened' | 'changed' | 'closed'; document: DocumentSnapshot }
export interface Context { subscriptions: Disposable[] }
export interface Graphite {
  commands: {
    registerCommand(id: string, handler: (arguments_: unknown, signal: AbortSignal) => unknown | Promise<unknown>): Promise<Disposable>;
  };
  documents: {
    list(): Promise<DocumentSnapshot[]>;
    snapshot(uri: string): Promise<DocumentSnapshot>;
    applyEdits(edits: DocumentEdit[]): Promise<void>;
    onDidChange(listener: (event: DocumentEvent) => void | Promise<void>): Promise<Disposable>;
  };
  workspace: { get(): Promise<{ root: string }>; configuration(): Promise<unknown> };
  window: { showMessage(text: string): Promise<void> };
}
