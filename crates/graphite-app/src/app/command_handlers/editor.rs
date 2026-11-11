use super::*;

pub(super) fn delete_left(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().backspace();
    Ok(())
}

pub(super) fn delete_right(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().delete();
    Ok(())
}

pub(super) fn type_newline(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().insert_newline();
    Ok(())
}

pub(super) fn tab(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.expand_emmet_at_cursor() {
        return Ok(());
    }
    {
        let document = app.command_document();
        let tab = &mut app.documents[document];
        let indentation = if tab.indentation.insert_spaces {
            " ".repeat(tab.indentation.tab_size)
        } else {
            "\t".into()
        };
        app.editor_mut().insert(&indentation);
    };
    Ok(())
}

pub(super) fn emmet_expand(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if app.expand_emmet_at_cursor() {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "No supported Emmet abbreviation at the cursor".into(),
        ))
    }
}

pub(super) fn editor_action_toggle_indentation(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        let document = app.command_document();
        let mut target = app.documents[document].indentation;
        target.insert_spaces = !target.insert_spaces;
        let result = app.editor_mut().convert_indentation(target);
        match result {
            Ok(_) => {
                app.documents[document].indentation = target;
                app.status = app
                    .t(if target.insert_spaces {
                        "editor.indentation.spaces"
                    } else {
                        "editor.indentation.tabs"
                    })
                    .into();
            }
            Err(error) => return Err(CommandError::Failed(error.to_string())),
        }
    };
    Ok(())
}

pub(super) fn editor_action_start_find_replace_action(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.context_keys.close_widget(WidgetKind::Find);
    app.open_local_search(true);
    Ok(())
}

pub(super) fn editor_action_smart_select_expand(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.smart_select(true);
    Ok(())
}

pub(super) fn editor_action_smart_select_shrink(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.smart_select(false);
    Ok(())
}

pub(super) fn editor_action_add_selection_to_next_find_match(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.editor_mut().select_occurrences(false)
    };
    Ok(())
}

pub(super) fn editor_action_add_selection_to_previous_find_match(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().select_previous_occurrence();
    Ok(())
}

pub(super) fn editor_action_duplicate_selection(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().duplicate_selection();
    Ok(())
}

pub(super) fn editor_action_add_cursor_above(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.add_cursors_vertical(false);
    Ok(())
}

pub(super) fn editor_action_add_cursor_below(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.add_cursors_vertical(true);
    Ok(())
}

pub(super) fn editor_action_add_cursors_to_line_ends(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.add_cursors_to_line_ends();
    Ok(())
}

pub(super) fn editor_action_select_highlights(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().select_occurrences(true);
    Ok(())
}

pub(super) fn editor_action_move_lines_up_action(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().move_line(-1);
    Ok(())
}

pub(super) fn editor_action_move_lines_down_action(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().move_line(1);
    Ok(())
}

pub(super) fn editor_action_copy_lines_up_action(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().duplicate_line(false);
    Ok(())
}

pub(super) fn editor_action_copy_lines_down_action(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().duplicate_line(true);
    Ok(())
}

pub(super) fn editor_action_delete_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().delete_line();
    Ok(())
}

pub(super) fn editor_action_sort_lines_ascending(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().sort_lines();
    Ok(())
}

pub(super) fn editor_action_remove_duplicate_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().deduplicate_lines();
    Ok(())
}

pub(super) fn undo(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    {
        app.editor_mut().undo();
    };
    Ok(())
}

pub(super) fn redo(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    {
        app.editor_mut().redo();
    };
    Ok(())
}

pub(super) fn editor_action_select_all(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        let mut buffer = app.editor_mut();
        let len = buffer.len_chars();
        buffer.set_selection(0, len);
    };
    Ok(())
}

pub(super) fn editor_action_clipboard_copy_action(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let command = invocation.request.id.as_str();
    {
        let mut clipboard = crate::app::clipboard::SystemClipboard;
        let copied = crate::app::clipboard::copy_or_cut(
            &mut app.editor_mut(),
            &mut clipboard,
            command == "editor.action.clipboardCutAction",
        );
        if !copied {
            return Err(CommandError::Failed(
                app.t("editor.clipboard.write_failed").into(),
            ));
        }
    };
    Ok(())
}

pub(super) fn editor_action_clipboard_paste_action(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        let mut clipboard = crate::app::clipboard::SystemClipboard;
        if !crate::app::clipboard::paste(&mut app.editor_mut(), &mut clipboard) {
            return Err(CommandError::Failed(
                app.t("editor.clipboard.read_failed").into(),
            ));
        }
    };
    Ok(())
}

pub(super) fn editor_action_trigger_suggest(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.completion_items.clear();
        if !app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Completion) {
            app.request_lsp("completion");
        }
    };
    Ok(())
}

pub(super) fn editor_action_reveal_definition(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.request_lsp("definition");
    Ok(())
}

pub(super) fn editor_action_show_hover(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if !app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Hover) {
        app.request_lsp("hover");
    }
    Ok(())
}

fn format_document(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Format) {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "No active VSIX formatter matches this document".into(),
        ))
    }
}

pub(super) fn editor_action_quick_fix(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.request_quick_fixes();
    Ok(())
}

pub(super) fn editor_action_marker_next(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.navigate_diagnostic(1);
    Ok(())
}

pub(super) fn editor_action_marker_prev(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.navigate_diagnostic(-1);
    Ok(())
}

fn move_cursor(
    app: &mut App,
    motion: graphite_text::CursorMotion,
    extend: bool,
) -> Result<(), CommandError> {
    app.command_view_mut().smart_selection_history.clear();
    app.command_view_mut().smart_selection_guard = None;
    if app.word_wrap
        && matches!(
            motion,
            graphite_text::CursorMotion::Up | graphite_text::CursorMotion::Down
        )
        && app.move_visual_selections(matches!(motion, graphite_text::CursorMotion::Down), extend)
    {
        return Ok(());
    }
    app.editor_mut().move_selections(motion, extend);
    Ok(())
}

pub(super) fn cursor_left(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Left, false)
}

pub(super) fn cursor_left_select(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Left, true)
}

pub(super) fn cursor_right(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Right, false)
}

pub(super) fn cursor_right_select(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Right, true)
}

pub(super) fn cursor_up(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Up, false)
}

pub(super) fn cursor_up_select(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Up, true)
}

pub(super) fn cursor_down(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Down, false)
}

pub(super) fn cursor_down_select(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Down, true)
}

pub(super) fn cursor_home(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Home, false)
}

pub(super) fn cursor_home_select(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::Home, true)
}

pub(super) fn cursor_end(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::End, false)
}

pub(super) fn cursor_end_select(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_cursor(app, graphite_text::CursorMotion::End, true)
}

pub(super) fn register(registry: &mut CommandRegistry) {
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
        "graphite.editor.duplicateSelection",
        "Duplicate Selection",
        "Editor",
        VIEW,
        "editorWritable",
        None,
        editor_action_duplicate_selection,
    );
    add(
        registry,
        "graphite.editor.addCursorAbove",
        "Add Cursor Above",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_cursor_above,
    );
    add(
        registry,
        "graphite.editor.addCursorBelow",
        "Add Cursor Below",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        editor_action_add_cursor_below,
    );
    add(
        registry,
        "graphite.editor.addCursorsToLineEnds",
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
        registry, "undo", "Undo", "Editor", VIEW, "canUndo", None, undo,
    );
    add(
        registry, "redo", "Redo", "Editor", VIEW, "canRedo", None, redo,
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
}
