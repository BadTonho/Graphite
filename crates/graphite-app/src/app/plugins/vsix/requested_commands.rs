//! Commands that extensions asked Graphite to run and Graphite does not have.
//!
//! An extension that calls `commands.executeCommand("some.command")` gets an error when Graphite
//! lacks the command, and nothing shows it to the user. Keeping the ids and how often they were
//! asked gives a list of what to implement for the extension to work.

use super::*;
use std::collections::BTreeMap;

/// Where a missing command was asked from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Source {
    /// `commands.executeCommand` called by extension code.
    Extension,
    /// The command of a code lens that was run.
    CodeLens,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::Extension => "extension",
            Source::CodeLens => "code lens",
        }
    }
}

#[derive(Default)]
pub(super) struct Requested {
    commands: BTreeMap<String, (u32, Source)>,
}

impl Requested {
    /// Counts one request; returns `true` the first time this id is seen.
    pub(super) fn record(&mut self, id: &str, source: Source) -> bool {
        let entry = self.commands.entry(id.to_owned());
        let first = matches!(entry, std::collections::btree_map::Entry::Vacant(_));
        let (count, kept) = entry.or_insert((0, source));
        *count += 1;
        // An id asked by extension code is the stronger signal, so it wins over a lens.
        *kept = (*kept).min(source);
        first
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// One `count<TAB>id<TAB>source` line per command, most requested first.
    pub(super) fn lines(&self) -> Vec<String> {
        let mut rows = self.commands.iter().collect::<Vec<_>>();
        rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(b.0)));
        rows.into_iter()
            .map(|(id, (count, source))| format!("{count}\t{id}\t{}", source.label()))
            .collect()
    }
}

impl App {
    /// Notes a command Graphite does not have. Each id is also appended once to
    /// `<profile>/code-oss/requested-commands.log`, so the list survives a restart.
    pub(super) fn record_requested_vsix_command(
        &self,
        runtime: &mut Runtime,
        id: &str,
        source: Source,
    ) {
        if !runtime.requested.record(id, source) {
            return;
        }
        let directory = self.profile_root.join("code-oss");
        if std::fs::create_dir_all(&directory).is_err() {
            return;
        }
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("requested-commands.log"))
        {
            let _ = writeln!(file, "{id}");
        }
    }

    /// Opens the list of requested-but-missing commands in a read-only view.
    pub(crate) fn show_requested_vsix_commands(&mut self) {
        let lines = self
            .plugins
            .vsix
            .as_ref()
            .map(|runtime| runtime.requested.lines())
            .unwrap_or_default();
        let text = if lines.is_empty() {
            self.t("plugins.requested_commands.none").to_owned()
        } else {
            format!(
                "{}\n\n{}\n",
                self.t("plugins.requested_commands.header"),
                lines.join("\n")
            )
        };
        let path = self.profile_root.join("vsix-requested-commands.txt");
        match self.documents.id_for_path(&path) {
            Some(id) => {
                let _ = self.documents[id].buffer.reload_text(&text);
                self.activate_editor_document(id);
            }
            None => self.open_read_only_view(path, &text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;

    #[test]
    fn counts_requests_and_lists_the_most_asked_first() {
        let mut requested = Requested::default();
        assert!(requested.is_empty());
        assert!(requested.record("b.cmd", Source::Extension));
        assert!(!requested.record("b.cmd", Source::Extension));
        assert!(requested.record("a.cmd", Source::CodeLens));
        assert!(requested.record("c.cmd", Source::Extension));
        assert!(!requested.record("b.cmd", Source::CodeLens));
        assert_eq!(
            requested.lines(),
            vec![
                "3\tb.cmd\textension",
                "1\ta.cmd\tcode lens",
                "1\tc.cmd\textension"
            ]
        );
    }

    #[test]
    fn a_request_from_extension_code_outranks_a_lens_for_the_same_command() {
        let mut requested = Requested::default();
        requested.record("x", Source::CodeLens);
        requested.record("x", Source::Extension);
        assert_eq!(requested.lines(), vec!["2\tx\textension"]);
    }

    fn app_with_runtime(dir: &TempDir) -> (App, LoopbackHost) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        let (session, host) = Session::loopback();
        app.plugins.vsix = Some(Runtime::for_test(session));
        (app, host)
    }

    #[test]
    fn an_unknown_command_from_an_extension_is_refused_and_listed() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_runtime(&dir);
        let mut runtime = app.plugins.vsix.take().unwrap();
        let request = json!({
            "id": 7, "actor": "MainThreadCommands", "method": "$executeCommand",
            "args": ["ext.doesNotExist", []]
        });
        app.handle_vsix_request(&mut runtime, &request).unwrap();
        app.plugins.vsix = Some(runtime);

        let reply = host.next_sent().expect("a reply");
        assert!(reply["error"]
            .as_str()
            .unwrap()
            .contains("ext.doesNotExist"));

        app.show_requested_vsix_commands();
        assert!(app.editor().text().contains("ext.doesNotExist"));
        let log =
            std::fs::read_to_string(dir.path().join("profile/code-oss/requested-commands.log"))
                .unwrap();
        assert_eq!(log.trim(), "ext.doesNotExist");
    }

    #[test]
    fn the_list_is_empty_until_something_is_requested() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_runtime(&dir);
        app.show_requested_vsix_commands();
        assert_eq!(
            app.editor().text().trim(),
            app.t("plugins.requested_commands.none")
        );
    }

    #[test]
    fn a_code_lens_command_that_does_not_exist_is_listed() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_runtime(&dir);
        app.run_lens_command(code_lens::LensCommand {
            id: "editor.action.showReferences".into(),
            title: "2 references".into(),
            args: Vec::new(),
        });
        assert!(app.status.contains("editor.action.showReferences"));
        app.show_requested_vsix_commands();
        assert!(app.editor().text().contains("editor.action.showReferences"));
        assert!(app.editor().text().contains("code lens"));
    }
}
