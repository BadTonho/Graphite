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
    // Formatters that list the line feed among their trigger characters run after Enter too.
    app.format_on_type("\n");
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

pub(super) fn editor_action_join_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().join_lines();
    Ok(())
}

pub(super) fn editor_action_indent_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let document = app.command_document();
    let indentation = app.documents[document].indentation;
    app.editor_mut().indent_lines(indentation);
    Ok(())
}

pub(super) fn editor_action_outdent_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let document = app.command_document();
    let indentation = app.documents[document].indentation;
    app.editor_mut().outdent_lines(indentation);
    Ok(())
}

pub(super) fn editor_action_reverse_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().reverse_lines();
    Ok(())
}

pub(super) fn editor_action_sort_lines_descending(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().sort_lines_ordered(true);
    Ok(())
}

pub(super) fn editor_action_transpose(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().transpose_characters();
    Ok(())
}

pub(super) fn editor_action_transform_upper(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Upper);
    Ok(())
}

pub(super) fn editor_action_transform_lower(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Lower);
    Ok(())
}

pub(super) fn editor_action_transform_title(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Title);
    Ok(())
}

pub(super) fn editor_action_transform_snake(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Snake);
    Ok(())
}

pub(super) fn editor_action_transform_camel(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Camel);
    Ok(())
}

pub(super) fn editor_action_transform_pascal(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Pascal);
    Ok(())
}

pub(super) fn editor_action_transform_kebab(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut()
        .transform_case(graphite_text::CaseTransform::Kebab);
    Ok(())
}

pub(super) fn editor_toggle_fold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_fold();
    Ok(())
}

pub(super) fn editor_fold_recursively(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_recursively();
    Ok(())
}

pub(super) fn editor_unfold_recursively(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.unfold_recursively();
    Ok(())
}

pub(super) fn editor_toggle_fold_recursively(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_fold_recursively();
    Ok(())
}

pub(super) fn editor_fold_level_1(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(1);
    Ok(())
}

pub(super) fn editor_fold_level_2(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(2);
    Ok(())
}

pub(super) fn editor_fold_level_3(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(3);
    Ok(())
}

pub(super) fn editor_fold_level_4(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(4);
    Ok(())
}

pub(super) fn editor_fold_level_5(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(5);
    Ok(())
}

pub(super) fn editor_fold_level_6(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(6);
    Ok(())
}

pub(super) fn editor_fold_level_7(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_level(7);
    Ok(())
}

pub(super) fn editor_fold_all_except(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_all_except_selection();
    Ok(())
}

pub(super) fn editor_unfold_all_except(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.unfold_all_except_selection();
    Ok(())
}

pub(super) fn editor_fold_all_block_comments(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_all_block_comments();
    Ok(())
}

pub(super) fn editor_fold_all_marker_regions(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_marker_regions(true);
    Ok(())
}

pub(super) fn editor_unfold_all_marker_regions(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_marker_regions(false);
    Ok(())
}

pub(super) fn editor_goto_next_fold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.go_to_next_fold();
    Ok(())
}

pub(super) fn editor_goto_previous_fold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.go_to_previous_fold();
    Ok(())
}

pub(super) fn editor_goto_parent_fold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.go_to_parent_fold();
    Ok(())
}

pub(super) fn editor_create_fold_range(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.create_fold_range_from_selection();
    Ok(())
}

pub(super) fn editor_remove_manual_folds(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.remove_manual_fold_ranges();
    Ok(())
}

pub(super) fn editor_toggle_import_fold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_import_fold();
    Ok(())
}

pub(super) fn editor_add_cursors_to_bottom(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().add_cursors_to_edge(true);
    Ok(())
}

pub(super) fn editor_add_cursors_to_top(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().add_cursors_to_edge(false);
    Ok(())
}

pub(super) fn editor_change_all(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().select_occurrences(true);
    Ok(())
}

pub(super) fn editor_focus_next_cursor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().focus_cursor(true);
    Ok(())
}

pub(super) fn editor_focus_previous_cursor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().focus_cursor(false);
    Ok(())
}

pub(super) fn editor_move_selection_to_next_match(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().move_selection_to_occurrence(true);
    Ok(())
}

pub(super) fn editor_move_selection_to_previous_match(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().move_selection_to_occurrence(false);
    Ok(())
}

pub(super) fn editor_set_selection_anchor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_selection_anchor();
    Ok(())
}

pub(super) fn editor_go_to_selection_anchor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if app.editor_mut().go_to_selection_anchor() {
        Ok(())
    } else {
        Err(CommandError::Disabled(app.t("editor.anchor.none").into()))
    }
}

pub(super) fn editor_select_from_anchor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if app.editor_mut().select_from_selection_anchor() {
        Ok(())
    } else {
        Err(CommandError::Disabled(app.t("editor.anchor.none").into()))
    }
}

pub(super) fn editor_cancel_selection_anchor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.cancel_selection_anchor();
    Ok(())
}

pub(super) fn editor_move_selected_text_left(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().move_selected_text(true);
    Ok(())
}

pub(super) fn editor_move_selected_text_right(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().move_selected_text(false);
    Ok(())
}

pub(super) fn editor_transpose_letters(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().transpose_letters();
    Ok(())
}

pub(super) fn editor_select_to_bracket(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().select_to_bracket(true);
    Ok(())
}

pub(super) fn editor_remove_brackets(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().remove_brackets();
    Ok(())
}

pub(super) fn editor_replace_previous_value(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().replace_with_next_value(false);
    Ok(())
}

pub(super) fn editor_replace_next_value(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().replace_with_next_value(true);
    Ok(())
}

pub(super) fn editor_font_zoom_in(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.run_font_zoom(1);
    Ok(())
}

pub(super) fn editor_font_zoom_out(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.run_font_zoom(-1);
    Ok(())
}

pub(super) fn editor_font_zoom_reset(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.run_font_zoom(0);
    Ok(())
}

pub(super) fn editor_jump_to_bracket(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().jump_to_bracket();
    Ok(())
}

pub(super) fn editor_find_references(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_references();
    Ok(())
}

pub(super) fn editor_reference_search(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_references();
    Ok(())
}

pub(super) fn editor_toggle_sticky_scroll(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_sticky_scroll();
    Ok(())
}

pub(super) fn editor_focus_sticky_scroll(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.focus_sticky_scroll();
    Ok(())
}

pub(super) fn editor_select_next_sticky_scroll_line(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.select_sticky_scroll_line(true);
    Ok(())
}

pub(super) fn editor_select_previous_sticky_scroll_line(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.select_sticky_scroll_line(false);
    Ok(())
}

pub(super) fn editor_go_to_focused_sticky_scroll_line(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.go_to_focused_sticky_scroll_line();
    Ok(())
}

pub(super) fn editor_select_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.focus_editor_text();
    Ok(())
}

pub(super) fn editor_announce_cursor_position(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.announce_cursor_position();
    Ok(())
}

pub(super) fn editor_action_go_to_references(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_references();
    Ok(())
}

pub(super) fn editor_action_go_to_implementations(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_implementations();
    Ok(())
}

pub(super) fn editor_action_peek_definition(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_peek_definition();
    Ok(())
}

pub(super) fn editor_action_peek_declaration(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_peek_declaration();
    Ok(())
}

pub(super) fn editor_action_peek_type_definition(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_peek_type_definition();
    Ok(())
}

pub(super) fn editor_action_peek_implementation(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_peek_implementation();
    Ok(())
}

pub(super) fn editor_action_toggle_overtype_insert_mode(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.overtype_mode = !app.overtype_mode;
    Ok(())
}

fn colon_indents_for_active_document(app: &App) -> bool {
    matches!(
        app.documents[app.command_document()].language,
        Some(graphite_syntax::Language::Python | graphite_syntax::Language::Yaml)
    )
}

pub(super) fn editor_action_reindent_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let style = app.documents[app.command_document()].indentation;
    let colon_indents = colon_indents_for_active_document(app);
    let _ = app.editor_mut().reindent_lines(style, colon_indents);
    Ok(())
}

pub(super) fn editor_action_reindent_selected_lines(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let style = app.documents[app.command_document()].indentation;
    let colon_indents = colon_indents_for_active_document(app);
    let _ = app
        .editor_mut()
        .reindent_selected_lines(style, colon_indents);
    Ok(())
}

pub(super) fn editor_action_rename(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_rename();
    Ok(())
}

pub(super) fn editor_action_trigger_parameter_hints(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_signature_help();
    Ok(())
}

pub(super) fn graphite_lsp_show_call_hierarchy(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_call_hierarchy();
    Ok(())
}

pub(super) fn graphite_lsp_show_type_hierarchy(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.lsp_type_hierarchy();
    Ok(())
}

pub(super) fn editor_fold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_at_cursor();
    Ok(())
}

pub(super) fn editor_unfold(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.unfold_at_cursor();
    Ok(())
}

pub(super) fn editor_fold_all(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.fold_all();
    Ok(())
}

pub(super) fn editor_toggle_pin_tab(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_pin_active_view();
    Ok(())
}

pub(super) fn editor_unfold_all(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.unfold_all();
    Ok(())
}

pub(super) fn undo(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.route_custom_editor_action(crate::app::plugins::vsix::EditorAction::Undo) {
        return Ok(());
    }
    {
        app.editor_mut().undo();
    };
    Ok(())
}

pub(super) fn redo(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.route_custom_editor_action(crate::app::plugins::vsix::EditorAction::Redo) {
        return Ok(());
    }
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
        // Format on paste covers the pasted text only when a single cursor pastes it.
        let start = app
            .editor()
            .selection()
            .map_or_else(|| app.editor().cursor().char_idx, |s| s.anchor.min(s.head));
        let single_cursor = app.editor().selections().len() <= 1;
        let mut clipboard = crate::app::clipboard::SystemClipboard;
        if !crate::app::clipboard::paste(&mut app.editor_mut(), &mut clipboard) {
            return Err(CommandError::Failed(
                app.t("editor.clipboard.read_failed").into(),
            ));
        }
        let end = app.editor().cursor().char_idx;
        if app.format_on_paste && single_cursor && end > start {
            app.lsp_format_chars(start, end);
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
        if !app.request_native_language(graphite_core_api::LanguageProviderKind::Completion)
            && !app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Completion)
        {
            app.request_lsp("completion");
        }
    };
    Ok(())
}

pub(super) fn editor_action_reveal_definition(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if !app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Definition) {
        app.request_lsp("definition");
    }
    Ok(())
}

/// Declarations and type definitions are answered only by extension providers.
fn vsix_only_navigation(
    app: &mut App,
    feature: crate::app::plugins::VsixLanguageFeature,
) -> Result<(), CommandError> {
    if app.request_vsix_language(feature) {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "No active VSIX provider matches this document".into(),
        ))
    }
}

pub(super) fn editor_action_go_to_declaration(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    vsix_only_navigation(app, crate::app::plugins::VsixLanguageFeature::Declaration)
}

pub(super) fn editor_action_go_to_type_definition(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    vsix_only_navigation(
        app,
        crate::app::plugins::VsixLanguageFeature::TypeDefinition,
    )
}

pub(super) fn editor_action_show_color_picker(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.start_color_picker().map_err(CommandError::Failed)
}

pub(super) fn editor_action_show_hover(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if !app.request_native_language(graphite_core_api::LanguageProviderKind::Hover)
        && !app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Hover)
    {
        app.request_lsp("hover");
    }
    // What decorations show for the range under the cursor appears even when no provider answers.
    if let Some(extra) = app.decoration_hover_at_cursor() {
        if app.hover_text.as_deref().is_none_or(str::is_empty) {
            app.hover_text = Some(extra);
        }
    }
    Ok(())
}

/// Prefers an active native/VSIX formatter and falls back to the language server.
fn format_document(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.request_native_language(graphite_core_api::LanguageProviderKind::Formatter) {
        return Ok(());
    }
    if app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::Format) {
        return Ok(());
    }
    if app.lsp_format_document() {
        return Ok(());
    }
    Err(CommandError::Disabled(
        "No active VSIX formatter or language server matches this document".into(),
    ))
}

/// Prefers an active VSIX range formatter and falls back to the language server.
fn format_selection(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::FormatRange) {
        return Ok(());
    }
    if app.lsp_format_selection() {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "No language server matches this document".into(),
        ))
    }
}

fn editor_action_refactor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if app.request_refactors() {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "No language server matches this document".into(),
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
}

fn show_requested_commands(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.show_requested_vsix_commands();
    Ok(())
}

fn run_code_lens(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.run_code_lens_on_cursor_line() {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "There is no code lens on the cursor line".into(),
        ))
    }
}
