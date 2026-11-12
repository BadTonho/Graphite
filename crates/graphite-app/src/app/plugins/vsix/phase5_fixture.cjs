const vscode = require('vscode');

exports.activate = context => {
    const selector = { language: 'plaintext', scheme: 'file' };
    const first = new vscode.Range(0, 0, 0, 1);
    const diagnostics = vscode.languages.createDiagnosticCollection('phase5');
    context.subscriptions.push(
        diagnostics,
        vscode.languages.registerHoverProvider(selector, {
            provideHover: () => new vscode.Hover(new vscode.MarkdownString('VSIX hover')),
        }),
        vscode.languages.registerCompletionItemProvider(selector, {
            provideCompletionItems: () => [new vscode.CompletionItem('VSIX completion')],
        }),
        vscode.languages.registerDocumentFormattingEditProvider(selector, {
            provideDocumentFormattingEdits: document => [
                vscode.TextEdit.replace(document.lineAt(0).range, 'Formatted'),
            ],
        }),
        vscode.languages.registerDefinitionProvider(selector, {
            provideDefinition: document => new vscode.Location(document.uri, new vscode.Range(0, 1, 0, 3)),
        }),
        vscode.languages.registerDeclarationProvider(selector, {
            provideDeclaration: document => new vscode.Location(document.uri, new vscode.Range(0, 2, 0, 3)),
        }),
        vscode.languages.registerImplementationProvider(selector, {
            provideImplementation: document => [
                new vscode.Location(document.uri, new vscode.Range(0, 0, 0, 1)),
                new vscode.Location(document.uri, new vscode.Range(0, 3, 0, 4)),
            ],
        }),
        vscode.languages.registerTypeDefinitionProvider(selector, {
            provideTypeDefinition: document => new vscode.Location(document.uri, new vscode.Range(0, 3, 0, 4)),
        }),
        vscode.languages.registerReferenceProvider(selector, {
            provideReferences: document => [
                new vscode.Location(document.uri, new vscode.Range(0, 0, 0, 1)),
                new vscode.Location(document.uri, new vscode.Range(0, 2, 0, 3)),
            ],
        }),
        vscode.languages.registerDocumentSymbolProvider(selector, {
            provideDocumentSymbols: document => [new vscode.DocumentSymbol(
                'Outer', 'detail', vscode.SymbolKind.Class,
                new vscode.Range(0, 0, 0, 6), new vscode.Range(0, 0, 0, 3),
            )],
        }),
        vscode.languages.registerCodeActionsProvider(selector, {
            provideCodeActions: document => {
                const action = new vscode.CodeAction('Uppercase first line', vscode.CodeActionKind.QuickFix);
                action.edit = new vscode.WorkspaceEdit();
                action.edit.replace(document.uri, document.lineAt(0).range, document.lineAt(0).text.toUpperCase());
                return [action];
            },
        }),
        vscode.languages.registerRenameProvider(selector, {
            provideRenameEdits: (document, position, newName) => {
                const edit = new vscode.WorkspaceEdit();
                edit.replace(document.uri, document.lineAt(0).range, newName);
                return edit;
            },
        }),
        vscode.languages.registerSignatureHelpProvider(selector, {
            provideSignatureHelp: () => {
                const help = new vscode.SignatureHelp();
                const signature = new vscode.SignatureInformation('call(first, second)');
                signature.parameters = [new vscode.ParameterInformation('first'), new vscode.ParameterInformation('second')];
                help.signatures = [signature];
                help.activeParameter = 1;
                return help;
            },
        }),
        vscode.commands.registerCommand('phase5.start', async () => {
            const document = vscode.window.activeTextEditor.document;
            const folder = vscode.workspace.getWorkspaceFolder(document.uri);
            if (!folder) throw new Error('Missing workspace folder');
            const setting = vscode.workspace.getConfiguration('phase5').get('setting');
            const updated = new Promise(resolve => {
                const listener = vscode.workspace.onDidChangeConfiguration(event => {
                    if (event.affectsConfiguration('phase5.probe')) {
                        listener.dispose();
                        resolve();
                    }
                });
            });
            await vscode.workspace.getConfiguration('phase5').update('probe', true, vscode.ConfigurationTarget.Workspace);
            await updated;
            if (!vscode.workspace.getConfiguration('phase5').get('probe')) throw new Error('Missing configuration update');
            const input = await vscode.window.showInputBox({ prompt: 'Cancel this input', password: true });
            if (input !== undefined) throw new Error('Expected cancellation');
            const pick = await vscode.window.showQuickPick(['selected']);
            const edit = new vscode.WorkspaceEdit();
            edit.replace(document.uri, document.lineAt(0).range, setting + '-' + pick);
            if (!await vscode.workspace.applyEdit(edit)) throw new Error('Workspace edit rejected');
            diagnostics.set(document.uri, [new vscode.Diagnostic(first, 'VSIX diagnostic')]);
        }),
        vscode.commands.registerCommand('phase5.pending', async () => {
            const result = await vscode.window.showInputBox({ prompt: 'Revoke trust while waiting' });
            await vscode.window.activeTextEditor.edit(edit => edit.insert(first.start, result ?? 'stale'));
        }),
        vscode.commands.registerCommand('phase5.workbench', async () => {
            const channel = vscode.window.createOutputChannel('Phase 5 log');
            channel.appendLine('workbench line');
            channel.show(true);
            const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 10);
            item.text = 'Phase 5 status';
            item.command = 'phase5.statusclick';
            item.show();
            const found = await vscode.workspace.findFiles('**/*.txt', '**/ignored/**', 10);
            if (!found.some(uri => uri.fsPath.endsWith('sample.txt'))) throw new Error('findFiles missing sample');
            const hits = [];
            await vscode.workspace.findTextInFiles({ pattern: 'sample' }, { include: '**/*.txt' }, hit => hits.push(hit));
            if (!hits.some(hit => hit.uri.fsPath.endsWith('sample.txt') && hit.preview.text.includes('sample'))) {
                throw new Error('findTextInFiles missing sample');
            }
            const external = await vscode.env.asExternalUri(vscode.Uri.parse('https://example.com/'));
            if (!external.toString().startsWith('https://')) throw new Error('asExternalUri changed the link');
        }),
        vscode.commands.registerCommand('phase5.statusclick', () => {
            vscode.window.showInformationMessage('status clicked');
        }),
        vscode.commands.registerCommand('phase5.watch', async () => {
            const folder = vscode.workspace.workspaceFolders[0];
            const watcher = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(folder, '*.watched'));
            const created = new Promise(resolve => watcher.onDidCreate(uri => resolve(uri)));
            const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error('watcher event missing')), 20000));
            const uri = await Promise.race([created, timeout]);
            watcher.dispose();
            if (!uri.fsPath.endsWith('new.watched')) throw new Error('watcher reported ' + uri.fsPath);
        }),
        vscode.commands.registerCommand('phase5.terminal', async () => {
            const windows = process.platform === 'win32';
            const terminal = vscode.window.createTerminal({
                name: 'Phase 5 terminal',
                shellPath: windows ? 'cmd.exe' : '/bin/sh',
                shellArgs: windows ? ['/C', 'exit 3'] : ['-c', 'exit 3'],
            });
            const closed = new Promise(resolve => {
                const listener = vscode.window.onDidCloseTerminal(closedTerminal => {
                    if (closedTerminal === terminal) {
                        listener.dispose();
                        resolve(closedTerminal.exitStatus);
                    }
                });
            });
            terminal.sendText('echo ignored', true);
            terminal.show(true);
            const status = await closed;
            if (!status || status.code !== 3) throw new Error('Terminal exit status missing');
        }),
        vscode.commands.registerCommand('phase5.crash', () => process.kill(process.pid)),
    );
};
