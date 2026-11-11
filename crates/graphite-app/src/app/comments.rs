use super::*;
use graphite_plugins::manifest::Metadata;
use graphite_text::TextReplacement;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommentTokens {
    pub line: Option<String>,
    pub block: Option<(String, String)>,
}

fn extension(path: &Path) -> Option<String> {
    path.extension()?.to_str().map(str::to_ascii_lowercase)
}

impl App {
    pub(crate) fn active_comment_tokens(&self) -> Option<CommentTokens> {
        let path = self.documents[self.active_document()]
            .buffer
            .path()?
            .to_path_buf();
        let extension = extension(&path)?;
        let native = match extension.as_str() {
            "rs" => Some((Some("//"), Some(("/*", "*/")))),
            "py" | "pyi" => Some((Some("#"), None)),
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => Some((Some("//"), Some(("/*", "*/")))),
            "jsonc" => Some((Some("//"), Some(("/*", "*/")))),
            _ => None,
        };
        if let Some((line, block)) = native {
            return Some(CommentTokens {
                line: line.map(str::to_owned),
                block: block.map(|(start, end)| (start.to_owned(), end.to_owned())),
            });
        }

        let matches = self
            .plugins
            .installed_plugins()
            .iter()
            .filter(|record| {
                record.enabled
                    && record.trusted_version.as_deref() == Some(record.metadata.version())
            })
            .filter_map(|record| match &record.metadata {
                Metadata::Vsix(vsix) => Some(vsix.language_comments.iter()),
                Metadata::Native(_) => None,
            })
            .flatten()
            .filter(|comments| comments.extensions.iter().any(|ext| ext == &extension))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return None;
        }
        let comments = matches[0];
        Some(CommentTokens {
            line: comments.line_comment.clone(),
            block: comments
                .block_comment
                .as_ref()
                .map(|markers| (markers[0].clone(), markers[1].clone())),
        })
    }

    pub(crate) fn toggle_line_comment(&mut self) {
        let Some(marker) = self.active_comment_tokens().and_then(|tokens| tokens.line) else {
            return;
        };
        let text = self.editor().text();
        let chars = text.chars().collect::<Vec<_>>();
        let starts = line_starts(&chars);
        let selection = self
            .editor()
            .selection()
            .unwrap_or(graphite_text::Selection {
                anchor: self.editor().cursor().char_idx,
                head: self.editor().cursor().char_idx,
            });
        let (start, end) = (
            selection.anchor.min(selection.head),
            selection.anchor.max(selection.head),
        );
        let first = line_for(&starts, start);
        let mut last = line_for(&starts, end.min(chars.len()));
        if end > start
            && starts
                .get(last)
                .is_some_and(|line_start| *line_start == end)
        {
            last = last.saturating_sub(1);
        }
        let lines = (first..=last).collect::<Vec<_>>();
        let nonblank = lines
            .iter()
            .filter_map(|line| {
                let line_start = starts[*line];
                let line_end = starts.get(*line + 1).copied().unwrap_or(chars.len());
                let line_end = if line_end > line_start && chars[line_end - 1] == '\n' {
                    line_end - 1
                } else {
                    line_end
                };
                let content = chars[line_start..line_end].iter().collect::<String>();
                let indent = content.chars().take_while(|ch| ch.is_whitespace()).count();
                (!content.trim().is_empty()).then_some((line_start, line_end, indent, content))
            })
            .collect::<Vec<_>>();
        let remove = !nonblank.is_empty()
            && nonblank.iter().all(|(_, _, indent, content)| {
                content
                    .chars()
                    .skip(*indent)
                    .collect::<String>()
                    .starts_with(&marker)
            });
        let edits = nonblank
            .into_iter()
            .map(|(line_start, _, indent, content)| {
                let at = line_start + indent;
                if remove {
                    let after = content.chars().skip(indent).collect::<String>();
                    let amount = marker.chars().count()
                        + usize::from(after[marker.len()..].starts_with(' '));
                    TextReplacement {
                        range: at..at + amount,
                        text: String::new(),
                    }
                } else {
                    TextReplacement {
                        range: at..at,
                        text: format!("{marker} "),
                    }
                }
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return;
        }
        let result = self.editor_mut().apply_transaction(&edits);
        if let Err(error) = result {
            self.status = error.to_string();
        }
    }

    pub(crate) fn toggle_block_comment(&mut self) {
        let Some((open, close)) = self.active_comment_tokens().and_then(|tokens| tokens.block)
        else {
            return;
        };
        let text = self.editor().text();
        let chars = text.chars().collect::<Vec<_>>();
        let selection = self.editor().selection();
        let (start, end) = if let Some(selection) = selection.filter(|s| s.anchor != s.head) {
            (
                selection.anchor.min(selection.head),
                selection.anchor.max(selection.head),
            )
        } else {
            let cursor = self.editor().cursor().char_idx.min(chars.len());
            let starts = line_starts(&chars);
            let line = line_for(&starts, cursor);
            let start = starts[line];
            let mut end = starts.get(line + 1).copied().unwrap_or(chars.len());
            if end > start && chars[end - 1] == '\n' {
                end -= 1;
            }
            (start, end)
        };
        if start == end {
            return;
        }
        let selected = chars[start..end].iter().collect::<String>();
        let open_chars = open.chars().count();
        let close_chars = close.chars().count();
        let edits = if selected.starts_with(&open)
            && selected.ends_with(&close)
            && selected.len() >= open.len() + close.len()
        {
            vec![
                TextReplacement {
                    range: start..start + open_chars,
                    text: String::new(),
                },
                TextReplacement {
                    range: end - close_chars..end,
                    text: String::new(),
                },
            ]
        } else {
            vec![
                TextReplacement {
                    range: end..end,
                    text: close,
                },
                TextReplacement {
                    range: start..start,
                    text: open,
                },
            ]
        };
        let result = self.editor_mut().apply_transaction(&edits);
        if let Err(error) = result {
            self.status = error.to_string();
        }
    }
}

fn line_starts(chars: &[char]) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        chars
            .iter()
            .enumerate()
            .filter_map(|(index, ch)| (*ch == '\n').then_some(index + 1)),
    );
    starts
}

fn line_for(starts: &[usize], position: usize) -> usize {
    starts
        .partition_point(|start| *start <= position)
        .saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_comment_toggles_indented_selection_and_keeps_blank_lines() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("sample.rs");
        std::fs::write(&file, "fn a() {\n    let x = 1;\n\n}").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(file);
        let len = app.editor().len_chars();
        app.editor_mut().set_selection(0, len);
        app.toggle_line_comment();
        assert!(app
            .editor()
            .text()
            .starts_with("// fn a() {\n    // let x = 1;\n\n// }"));
        app.toggle_line_comment();
        assert_eq!(app.editor().text(), "fn a() {\n    let x = 1;\n\n}");
    }

    #[test]
    fn block_comment_wraps_and_unwraps_selection() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("sample.js");
        std::fs::write(&file, "value").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(file);
        app.editor_mut().set_selection(0, 5);
        app.toggle_block_comment();
        assert_eq!(app.editor().text(), "/*value*/");
        let len = app.editor().len_chars();
        app.editor_mut().set_selection(0, len);
        app.toggle_block_comment();
        assert_eq!(app.editor().text(), "value");
    }

    #[test]
    fn comments_are_unavailable_for_plain_json_and_unknown_languages() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("sample.json");
        std::fs::write(&file, "{}").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(file);
        assert!(app.active_comment_tokens().is_none());
    }

    #[test]
    fn trusted_vsix_language_configuration_supplies_comment_markers() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("sample.lua");
        std::fs::write(&file, "print('hi')").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(file);
        let metadata = Metadata::Vsix(graphite_plugins::manifest::Vsix {
            id: "sample.comments".into(),
            version: "1.0.0".into(),
            description: String::new(),
            engines: serde_json::Value::Null,
            main: None,
            contributions: vec![],
            commands: vec![],
            activity_bar: vec![],
            language_comments: vec![graphite_plugins::manifest::LanguageComments {
                id: "lua".into(),
                extensions: vec!["lua".into()],
                line_comment: Some("--".into()),
                block_comment: None,
            }],
        });
        app.plugins
            .installed_plugins_mut()
            .push(graphite_plugins::store::Installed {
                metadata,
                enabled: true,
                trusted_version: Some("1.0.0".into()),
                directory: "sample.comments@1.0.0".into(),
            });
        assert_eq!(
            app.active_comment_tokens().unwrap().line.as_deref(),
            Some("--")
        );
    }
}
