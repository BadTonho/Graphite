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
  };
}
