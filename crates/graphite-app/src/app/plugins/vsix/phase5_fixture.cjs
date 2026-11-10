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
        vscode.commands.registerCommand('phase5.crash', () => process.kill(process.pid)),
    );
};
