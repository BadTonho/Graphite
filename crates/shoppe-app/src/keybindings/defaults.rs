//! Default command bindings; user entries are evaluated after these rules.
pub(super) const BINDINGS: &[(&str, &str, &str)] = &[
    ("ctrl+p", "workbench.action.quickOpen", "!terminalFocus"),
    ("ctrl+f", "actions.find", "editorTextFocus"),
    (
        "ctrl+h",
        "editor.action.startFindReplaceAction",
        "editorTextFocus",
    ),
    ("ctrl+s", "workbench.action.files.save", "editorTextFocus"),
    (
        "ctrl+w",
        "workbench.action.closeActiveEditor",
        "editorTextFocus",
    ),
    ("ctrl+tab", "workbench.action.nextEditor", "editorTextFocus"),
    ("ctrl+z", "undo", "editorTextFocus"),
    ("ctrl+y", "redo", "editorTextFocus"),
    ("ctrl+a", "editor.action.selectAll", "editorTextFocus"),
    (
        "ctrl+c",
        "editor.action.clipboardCopyAction",
        "editorTextFocus",
    ),
    (
        "ctrl+x",
        "editor.action.clipboardCutAction",
        "editorTextFocus",
    ),
    (
        "ctrl+v",
        "editor.action.clipboardPasteAction",
        "editorTextFocus",
    ),
    (
        "ctrl+d",
        "editor.action.addSelectionToNextFindMatch",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+l",
        "editor.action.selectHighlights",
        "editorTextFocus",
    ),
    (
        "alt+up",
        "editor.action.moveLinesUpAction",
        "editorTextFocus",
    ),
    (
        "alt+down",
        "editor.action.moveLinesDownAction",
        "editorTextFocus",
    ),
    (
        "shift+alt+up",
        "editor.action.copyLinesUpAction",
        "editorTextFocus",
    ),
    (
        "shift+alt+down",
        "editor.action.copyLinesDownAction",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+k",
        "editor.action.deleteLines",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+alt+s",
        "editor.action.sortLinesAscending",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+alt+d",
        "editor.action.removeDuplicateLines",
        "editorTextFocus",
    ),
    (
        "shift+alt+right",
        "editor.action.smartSelect.expand",
        "editorTextFocus",
    ),
    (
        "shift+alt+left",
        "editor.action.smartSelect.shrink",
        "editorTextFocus",
    ),
    (
        "ctrl+\\",
        "workbench.action.splitEditorRight",
        "editorTextFocus",
    ),
    (
        "ctrl+k ctrl+\\",
        "workbench.action.splitEditorDown",
        "editorTextFocus",
    ),
    (
        "ctrl+k ctrl+i",
        "editor.action.showHover",
        "editorTextFocus",
    ),
    ("f6", "workbench.action.focusNextGroup", "editorTextFocus"),
    ("f12", "editor.action.revealDefinition", "editorTextFocus"),
    ("f8", "editor.action.marker.next", "editorTextFocus"),
    ("shift+f8", "editor.action.marker.prev", "editorTextFocus"),
    (
        "ctrl+space",
        "editor.action.triggerSuggest",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+f",
        "workbench.action.findInFiles",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+p",
        "shoppe.action.insertSnippet",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+i",
        "shoppe.action.importVSCode",
        "editorTextFocus",
    ),
    (
        "ctrl+shift+alt+i",
        "editor.action.toggleIndentation",
        "editorTextFocus",
    ),
    ("ctrl+alt+t", "shoppe.action.toggleTheme", "editorTextFocus"),
    (
        "ctrl+`",
        "workbench.action.terminal.toggleTerminal",
        "!quickOpenVisible",
    ),
    (
        "ctrl+shift+m",
        "workbench.actions.view.problems",
        "!quickOpenVisible",
    ),
];
