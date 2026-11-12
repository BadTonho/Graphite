// Demo for Graphite's text decorations. Run "Graphite Demo: Decorate editor" from the command
// palette with any text file open; it marks the first line, every TODO and every line with the
// word "error", and adds a note after the last line.
const vscode = require('vscode');

let types = [];

function clear() {
    for (const type of types) type.dispose();
    types = [];
}

function decorate() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showInformationMessage('Open a text file first.');
        return;
    }
    clear();
    const document = editor.document;

    const header = vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        backgroundColor: 'rgba(56, 139, 253, 0.25)',
    });
    const todo = vscode.window.createTextEditorDecorationType({
        backgroundColor: 'rgba(255, 213, 0, 0.35)',
        border: '1px solid #ff9900',
    });
    const failing = vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        backgroundColor: { id: 'editorError.background' },
        after: { contentText: '   <- Graphite decoration', color: '#f14c4c' },
    });
    types = [header, todo, failing];

    const todoRanges = [];
    const errorRanges = [];
    for (let line = 0; line < document.lineCount; line++) {
        const text = document.lineAt(line).text;
        for (const match of text.matchAll(/TODO/g)) {
            todoRanges.push(new vscode.Range(line, match.index, line, match.index + 4));
        }
        if (/error/i.test(text)) errorRanges.push(new vscode.Range(line, 0, line, text.length));
    }
    editor.setDecorations(header, [new vscode.Range(0, 0, 0, 0)]);
    editor.setDecorations(todo, todoRanges);
    editor.setDecorations(failing, errorRanges);

    const last = document.lineCount - 1;
    const note = vscode.window.createTextEditorDecorationType({});
    types.push(note);
    editor.setDecorations(note, [{
        range: new vscode.Range(last, 0, last, 0),
        renderOptions: { after: { contentText: '   (note added by the extension)', color: '#3fb950' } },
    }]);
    vscode.window.showInformationMessage(
        `Decorated ${todoRanges.length} TODO and ${errorRanges.length} error line(s).`);
}

exports.activate = context => {
    context.subscriptions.push(
        vscode.commands.registerCommand('graphiteDemo.decorate', decorate),
        vscode.commands.registerCommand('graphiteDemo.clear', () => {
            clear();
            vscode.window.showInformationMessage('Decorations cleared.');
        }),
    );
};

exports.deactivate = clear;
