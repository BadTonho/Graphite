//! The table that registers every editor command with its title, category, and context.

use super::handlers::*;
use super::inspection::*;
use super::*;

pub(in super::super) fn register(registry: &mut CommandRegistry) {
    add(
        registry,
        "editor.emmet.action.expandAbbreviation",
        "Emmet: Expand Abbreviation",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        emmet_expand,
    );
    add(
        registry,
        "deleteLeft",
        "Delete left",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        delete_left,
    );
    add(
        registry,
        "deleteRight",
        "Delete right",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        delete_right,
    );
    add(
        registry,
        "typeNewline",
        "Type newline",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        type_newline,
    );
    add(
        registry,
        "tab",
        "Tab",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        tab,
    );
    add(
        registry,
        "editor.action.toggleIndentation",
        "Toggle indentation",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_toggle_indentation,
    );
    add(
        registry,
        "editor.action.startFindReplaceAction",
        "Start find replace action",
        "Editor",
        VIEW,
        "true",
        None,
        editor_action_start_find_replace_action,
    );
    add(
        registry,
        "editor.action.smartSelect.expand",
        "Expand",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_smart_select_expand,
    );
    add(
        registry,
        "editor.action.smartSelect.shrink",
        "Shrink",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_smart_select_shrink,
    );
    add(
        registry,
        "editor.action.addSelectionToNextFindMatch",
        "Add selection to next find match",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_selection_to_next_find_match,
    );
    add(
        registry,
        "editor.action.addSelectionToPreviousFindMatch",
        "Add selection to previous find match",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_selection_to_previous_find_match,
    );
    add(
        registry,
        "editor.action.duplicateSelection",
        "Duplicate Selection",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_duplicate_selection,
    );
    add(
        registry,
        "editor.action.insertCursorAbove",
        "Add Cursor Above",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_cursor_above,
    );
    add(
        registry,
        "editor.action.insertCursorBelow",
        "Add Cursor Below",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_cursor_below,
    );
    add(
        registry,
        "editor.action.insertCursorAtEndOfEachLineSelected",
        "Add Cursors to Line Ends",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_cursors_to_line_ends,
    );
    add(
        registry,
        "editor.action.toggleColumnSelection",
        "Toggle Column Selection Mode",
        "Editor",
        VIEW,
        "true",
        None,
        |app, _| {
            app.toggle_column_selection_mode();
            Ok(())
        },
    );
    add(
        registry,
        "graphite.editor.toggleMultiCursorModifier",
        "Switch Multi-Cursor Modifier",
        "Editor",
        VIEW,
        "true",
        None,
        |app, _| {
            app.toggle_multi_cursor_modifier();
            Ok(())
        },
    );
    add(
        registry,
        "editor.action.selectHighlights",
        "Select highlights",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_select_highlights,
    );
    add(
        registry,
        "editor.action.moveLinesUpAction",
        "Move lines up action",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_move_lines_up_action,
    );
    add(
        registry,
        "editor.action.moveLinesDownAction",
        "Move lines down action",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_move_lines_down_action,
    );
    add(
        registry,
        "editor.action.copyLinesUpAction",
        "Copy lines up action",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_copy_lines_up_action,
    );
    add(
        registry,
        "editor.action.copyLinesDownAction",
        "Copy lines down action",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_copy_lines_down_action,
    );
    add(
        registry,
        "editor.action.deleteLines",
        "Delete lines",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_delete_lines,
    );
    add(
        registry,
        "editor.action.sortLinesAscending",
        "Sort lines ascending",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_sort_lines_ascending,
    );
    add(
        registry,
        "editor.action.removeDuplicateLines",
        "Remove duplicate lines",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_remove_duplicate_lines,
    );
    add(
        registry,
        "editor.action.joinLines",
        "Join lines",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_join_lines,
    );
    add(
        registry,
        "editor.action.indentLines",
        "Indent line",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_indent_lines,
    );
    add(
        registry,
        "editor.action.outdentLines",
        "Outdent line",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_outdent_lines,
    );
    add(
        registry,
        "editor.action.reverseLines",
        "Reverse lines",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_reverse_lines,
    );
    add(
        registry,
        "editor.action.sortLinesDescending",
        "Sort lines descending",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_sort_lines_descending,
    );
    add(
        registry,
        "editor.action.transpose",
        "Transpose characters around the cursor",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transpose,
    );
    add(
        registry,
        "editor.action.transformToUppercase",
        "Transform to uppercase",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_upper,
    );
    add(
        registry,
        "editor.action.transformToLowercase",
        "Transform to lowercase",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_lower,
    );
    add(
        registry,
        "editor.action.transformToTitlecase",
        "Transform to title case",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_title,
    );
    add(
        registry,
        "editor.action.transformToSnakecase",
        "Transform to snake case",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_snake,
    );
    add(
        registry,
        "editor.action.transformToCamelcase",
        "Transform to camel case",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_camel,
    );
    add(
        registry,
        "editor.action.transformToPascalcase",
        "Transform to Pascal case",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_pascal,
    );
    add(
        registry,
        "editor.action.transformToKebabcase",
        "Transform to kebab case",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_transform_kebab,
    );
    add(
        registry,
        "editor.action.goToDeclaration",
        "Go to Declaration",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_go_to_declaration,
    );
    add(
        registry,
        "editor.action.goToTypeDefinition",
        "Go to Type Definition",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_go_to_type_definition,
    );
    add(
        registry,
        "editor.action.goToReferences",
        "Go to References",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_go_to_references,
    );
    add(
        registry,
        "editor.action.goToImplementation",
        "Go to Implementations",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_go_to_implementations,
    );
    add(
        registry,
        "editor.action.peekDefinition",
        "Peek Definition",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_peek_definition,
    );
    add(
        registry,
        "editor.action.peekDeclaration",
        "Peek Declaration",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_peek_declaration,
    );
    add(
        registry,
        "editor.action.peekTypeDefinition",
        "Peek Type Definition",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_peek_type_definition,
    );
    add(
        registry,
        "editor.action.peekImplementation",
        "Peek Implementation",
        "Go",
        VIEW,
        "true",
        None,
        editor_action_peek_implementation,
    );
    add(
        registry,
        "editor.action.toggleOvertypeInsertMode",
        "Toggle Overtype/Insert Mode",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_toggle_overtype_insert_mode,
    );
    add(
        registry,
        "editor.action.reindentlines",
        "Reindent Lines",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_reindent_lines,
    );
    add(
        registry,
        "editor.action.reindentselectedlines",
        "Reindent Selected Lines",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_reindent_selected_lines,
    );
    add(
        registry,
        "editor.action.rename",
        "Rename Symbol",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_rename,
    );
    add(
        registry,
        "editor.action.triggerParameterHints",
        "Trigger Parameter Hints",
        "Editor",
        VIEW,
        "true",
        None,
        editor_action_trigger_parameter_hints,
    );
    add(
        registry,
        "graphite.lsp.showCallHierarchy",
        "Show Call Hierarchy",
        "Go",
        VIEW,
        "true",
        None,
        graphite_lsp_show_call_hierarchy,
    );
    add(
        registry,
        "graphite.lsp.showTypeHierarchy",
        "Show Type Hierarchy",
        "Go",
        VIEW,
        "true",
        None,
        graphite_lsp_show_type_hierarchy,
    );
    add(
        registry, "undo", "Undo", "Editor", VIEW, "canUndo", None, undo,
    );
    add(
        registry, "redo", "Redo", "Editor", VIEW, "canRedo", None, redo,
    );
    add(
        registry,
        "editor.fold",
        "Fold",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold,
    );
    add(
        registry,
        "editor.unfold",
        "Unfold",
        "Editor",
        VIEW,
        "true",
        None,
        editor_unfold,
    );
    add(
        registry,
        "editor.foldAll",
        "Fold All",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_all,
    );
    add(
        registry,
        "editor.unfoldAll",
        "Unfold All",
        "Editor",
        VIEW,
        "true",
        None,
        editor_unfold_all,
    );
    add(
        registry,
        "editor.toggleFold",
        "Toggle fold",
        "Editor",
        VIEW,
        "true",
        None,
        editor_toggle_fold,
    );
    add(
        registry,
        "editor.foldRecursively",
        "Fold recursively",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_recursively,
    );
    add(
        registry,
        "editor.unfoldRecursively",
        "Unfold recursively",
        "Editor",
        VIEW,
        "true",
        None,
        editor_unfold_recursively,
    );
    add(
        registry,
        "editor.toggleFoldRecursively",
        "Toggle fold recursively",
        "Editor",
        VIEW,
        "true",
        None,
        editor_toggle_fold_recursively,
    );
    add(
        registry,
        "editor.foldLevel1",
        "Fold level 1",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_1,
    );
    add(
        registry,
        "editor.foldLevel2",
        "Fold level 2",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_2,
    );
    add(
        registry,
        "editor.foldLevel3",
        "Fold level 3",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_3,
    );
    add(
        registry,
        "editor.foldLevel4",
        "Fold level 4",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_4,
    );
    add(
        registry,
        "editor.foldLevel5",
        "Fold level 5",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_5,
    );
    add(
        registry,
        "editor.foldLevel6",
        "Fold level 6",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_6,
    );
    add(
        registry,
        "editor.foldLevel7",
        "Fold level 7",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_level_7,
    );
    add(
        registry,
        "editor.foldAllExcept",
        "Fold all regions except selected",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_all_except,
    );
    add(
        registry,
        "editor.unfoldAllExcept",
        "Unfold all regions except selected",
        "Editor",
        VIEW,
        "true",
        None,
        editor_unfold_all_except,
    );
    add(
        registry,
        "editor.foldAllBlockComments",
        "Fold all block comments",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_all_block_comments,
    );
    add(
        registry,
        "editor.foldAllMarkerRegions",
        "Fold all regions",
        "Editor",
        VIEW,
        "true",
        None,
        editor_fold_all_marker_regions,
    );
    add(
        registry,
        "editor.unfoldAllMarkerRegions",
        "Unfold all regions",
        "Editor",
        VIEW,
        "true",
        None,
        editor_unfold_all_marker_regions,
    );
    add(
        registry,
        "editor.gotoNextFold",
        "Go to next folding range",
        "Editor",
        VIEW,
        "true",
        None,
        editor_goto_next_fold,
    );
    add(
        registry,
        "editor.gotoPreviousFold",
        "Go to previous folding range",
        "Editor",
        VIEW,
        "true",
        None,
        editor_goto_previous_fold,
    );
    add(
        registry,
        "editor.gotoParentFold",
        "Go to parent fold",
        "Editor",
        VIEW,
        "true",
        None,
        editor_goto_parent_fold,
    );
    add(
        registry,
        "editor.createFoldingRangeFromSelection",
        "Create folding range from selection",
        "Editor",
        VIEW,
        "true",
        None,
        editor_create_fold_range,
    );
    add(
        registry,
        "editor.removeManualFoldingRanges",
        "Remove manual folding ranges",
        "Editor",
        VIEW,
        "true",
        None,
        editor_remove_manual_folds,
    );
    add(
        registry,
        "editor.toggleImportFold",
        "Toggle import fold",
        "Editor",
        VIEW,
        "true",
        None,
        editor_toggle_import_fold,
    );
    add(
        registry,
        "editor.action.addCursorsToBottom",
        "Add cursors to bottom",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_add_cursors_to_bottom,
    );
    add(
        registry,
        "editor.action.addCursorsToTop",
        "Add cursors to top",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_add_cursors_to_top,
    );
    add(
        registry,
        "editor.action.changeAll",
        "Change all occurrences",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_change_all,
    );
    add(
        registry,
        "editor.action.focusNextCursor",
        "Focus next cursor",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_focus_next_cursor,
    );
    add(
        registry,
        "editor.action.focusPreviousCursor",
        "Focus previous cursor",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_focus_previous_cursor,
    );
    add(
        registry,
        "editor.action.moveSelectionToNextFindMatch",
        "Move last selection to next find match",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_move_selection_to_next_match,
    );
    add(
        registry,
        "editor.action.moveSelectionToPreviousFindMatch",
        "Move last selection to previous find match",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_move_selection_to_previous_match,
    );
    add(
        registry,
        "editor.action.setSelectionAnchor",
        "Set selection anchor",
        "Editor",
        VIEW,
        "true",
        None,
        editor_set_selection_anchor,
    );
    add(
        registry,
        "editor.action.goToSelectionAnchor",
        "Go to selection anchor",
        "Editor",
        VIEW,
        "true",
        None,
        editor_go_to_selection_anchor,
    );
    add(
        registry,
        "editor.action.selectFromAnchorToCursor",
        "Select from anchor to cursor",
        "Editor",
        VIEW,
        "true",
        None,
        editor_select_from_anchor,
    );
    add(
        registry,
        "editor.action.cancelSelectionAnchor",
        "Cancel selection anchor",
        "Editor",
        VIEW,
        "true",
        None,
        editor_cancel_selection_anchor,
    );
    add(
        registry,
        "editor.action.moveCarretLeftAction",
        "Move selected text left",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_move_selected_text_left,
    );
    add(
        registry,
        "editor.action.moveCarretRightAction",
        "Move selected text right",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_move_selected_text_right,
    );
    add(
        registry,
        "editor.action.transposeLetters",
        "Transpose letters",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_transpose_letters,
    );
    add(
        registry,
        "editor.action.selectToBracket",
        "Select to bracket",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_select_to_bracket,
    );
    add(
        registry,
        "editor.action.removeBrackets",
        "Remove brackets",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_remove_brackets,
    );
    add(
        registry,
        "editor.action.inPlaceReplace.up",
        "Replace with previous value",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_replace_previous_value,
    );
    add(
        registry,
        "editor.action.inPlaceReplace.down",
        "Replace with next value",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_replace_next_value,
    );
    add(
        registry,
        "editor.action.fontZoomIn",
        "Increase editor font size",
        "Editor",
        VIEW,
        "true",
        None,
        editor_font_zoom_in,
    );
    add(
        registry,
        "editor.action.fontZoomOut",
        "Decrease editor font size",
        "Editor",
        VIEW,
        "true",
        None,
        editor_font_zoom_out,
    );
    add(
        registry,
        "editor.action.fontZoomReset",
        "Reset editor font size",
        "Editor",
        VIEW,
        "true",
        None,
        editor_font_zoom_reset,
    );
    add(
        registry,
        "editor.action.jumpToBracket",
        "Go to Bracket",
        "Go",
        VIEW,
        "documentAvailable",
        None,
        editor_jump_to_bracket,
    );
    add(
        registry,
        "editor.action.findReferences",
        "Find all references",
        "Go",
        VIEW,
        "true",
        None,
        editor_find_references,
    );
    add(
        registry,
        "editor.action.referenceSearch.trigger",
        "Find all references (search view)",
        "Go",
        VIEW,
        "true",
        None,
        editor_reference_search,
    );
    add(
        registry,
        "editor.action.toggleStickyScroll",
        "Toggle sticky scroll",
        "View",
        VIEW,
        "true",
        None,
        editor_toggle_sticky_scroll,
    );
    add(
        registry,
        "editor.action.focusStickyScroll",
        "Focus sticky scroll",
        "View",
        VIEW,
        "true",
        None,
        editor_focus_sticky_scroll,
    );
    add(
        registry,
        "editor.action.selectNextStickyScrollLine",
        "Select the next sticky scroll line",
        "View",
        VIEW,
        "true",
        None,
        editor_select_next_sticky_scroll_line,
    );
    add(
        registry,
        "editor.action.selectPreviousStickyScrollLine",
        "Select the previous sticky scroll line",
        "View",
        VIEW,
        "true",
        None,
        editor_select_previous_sticky_scroll_line,
    );
    add(
        registry,
        "editor.action.goToFocusedStickyScrollLine",
        "Go to the focused sticky scroll line",
        "View",
        VIEW,
        "true",
        None,
        editor_go_to_focused_sticky_scroll_line,
    );
    add(
        registry,
        "editor.action.selectEditor",
        "Focus the editor text",
        "View",
        VIEW,
        "true",
        None,
        editor_select_editor,
    );
    add(
        registry,
        "editor.action.announceCursorPosition",
        "Announce cursor position",
        "Editor",
        VIEW,
        "true",
        None,
        editor_announce_cursor_position,
    );
    add(
        registry,
        "editor.action.togglePinTab",
        "Pin or Unpin Tab",
        "Editor",
        VIEW,
        "true",
        None,
        editor_toggle_pin_tab,
    );
    add(
        registry,
        "editor.action.commentLine",
        "Toggle Line Comment",
        "Editor",
        VIEW,
        "editorWritable && canToggleLineComment",
        None,
        |app, _| {
            app.toggle_line_comment();
            Ok(())
        },
    );
    add(
        registry,
        "editor.action.blockComment",
        "Toggle Block Comment",
        "Editor",
        VIEW,
        "editorWritable && canToggleBlockComment",
        None,
        |app, _| {
            app.toggle_block_comment();
            Ok(())
        },
    );
    add(
        registry,
        "editor.action.selectAll",
        "Select all",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_select_all,
    );
    add(
        registry,
        "editor.action.clipboardCopyAction",
        "Clipboard copy action",
        "Editor",
        VIEW,
        "documentAvailable && editorHasSelection",
        None,
        editor_action_clipboard_copy_action,
    );
    add(
        registry,
        "editor.action.clipboardCutAction",
        "Clipboard cut action",
        "Editor",
        VIEW,
        "editorWritable && editorHasSelection",
        None,
        editor_action_clipboard_copy_action,
    );
    add(
        registry,
        "editor.action.clipboardPasteAction",
        "Clipboard paste action",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_clipboard_paste_action,
    );
    add(
        registry,
        "editor.action.triggerSuggest",
        "Trigger suggest",
        "Editor",
        VIEW,
        "documentHasPath",
        None,
        editor_action_trigger_suggest,
    );
    add(
        registry,
        "editor.action.revealDefinition",
        "Reveal definition",
        "Editor",
        VIEW,
        "documentHasPath",
        None,
        editor_action_reveal_definition,
    );
    add(
        registry,
        "editor.action.showOrFocusStandaloneColorPicker",
        "Show color picker",
        "Editor",
        VIEW,
        "documentHasPath",
        None,
        editor_action_show_color_picker,
    );
    add(
        registry,
        "editor.action.showHover",
        "Show hover",
        "Editor",
        VIEW,
        "documentHasPath",
        None,
        editor_action_show_hover,
    );
    add(
        registry,
        "editor.action.formatDocument",
        "Format document",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        format_document,
    );
    add(
        registry,
        "graphite.codeLens.run",
        "Run code lens on current line",
        "Editor",
        VIEW,
        "true",
        None,
        run_code_lens,
    );
    add(
        registry,
        "graphite.vsix.requestedCommands",
        "Show commands requested by extensions",
        "Editor",
        VIEW,
        "true",
        None,
        show_requested_commands,
    );
    add(
        registry,
        "editor.action.formatSelection",
        "Format selection",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        format_selection,
    );
    add(
        registry,
        "editor.action.refactor",
        "Refactor",
        "Editor",
        VIEW,
        "documentHasPath",
        None,
        editor_action_refactor,
    );
    add(
        registry,
        "editor.action.quickFix",
        "Quick fix",
        "Editor",
        VIEW,
        "documentHasPath",
        None,
        editor_action_quick_fix,
    );
    add(
        registry,
        "editor.action.marker.next",
        "Next",
        "Editor",
        ALL,
        "true",
        None,
        editor_action_marker_next,
    );
    add(
        registry,
        "editor.action.marker.prev",
        "Prev",
        "Editor",
        ALL,
        "true",
        None,
        editor_action_marker_prev,
    );
    add(
        registry,
        "cursorLeft",
        "Move left",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_left,
    );
    add(
        registry,
        "cursorLeftSelect",
        "Move left with selection",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_left_select,
    );
    add(
        registry,
        "cursorRight",
        "Move right",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_right,
    );
    add(
        registry,
        "cursorRightSelect",
        "Move right with selection",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_right_select,
    );
    add(
        registry,
        "cursorUp",
        "Move up",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_up,
    );
    add(
        registry,
        "cursorUpSelect",
        "Move up with selection",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_up_select,
    );
    add(
        registry,
        "cursorDown",
        "Move down",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_down,
    );
    add(
        registry,
        "cursorDownSelect",
        "Move down with selection",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_down_select,
    );
    add(
        registry,
        "cursorHome",
        "Move home",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_home,
    );
    add(
        registry,
        "cursorHomeSelect",
        "Move home with selection",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_home_select,
    );
    add(
        registry,
        "cursorEnd",
        "Move end",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_end,
    );
    add(
        registry,
        "cursorEndSelect",
        "Move end with selection",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        cursor_end_select,
    );
    add(
        registry,
        "editor.action.inspectTMScopes",
        "Inspect editor tokens and scopes",
        "Developer",
        VIEW,
        "documentAvailable",
        None,
        inspect_tm_scopes,
    );
    add(
        registry,
        "editor.action.toggleScreenReaderAccessibilityMode",
        "Toggle screen reader accessibility mode",
        "Editor",
        ALL,
        "true",
        None,
        toggle_screen_reader_mode,
    );
    add(
        registry,
        "workbench.action.showAccessibilityHelp",
        "Show accessibility help",
        "Help",
        ALL,
        "true",
        None,
        show_accessibility_help,
    );
}
