use super::*;
use crate::{
    app::command_palette::{CommandPalette, PaletteItem},
    file_format,
};

pub(crate) enum Review {
    Offer,
    List(Vec<Item>),
    Choose(Item),
    Confirm(Item),
}

pub(crate) struct Item {
    pub(super) path: PathBuf,
    title: String,
    timestamp: u64,
}

impl App {
    pub(super) fn draft_items(&mut self) -> Vec<Item> {
        let root = store::root(&self.profile_root);
        let tracked: HashSet<_> = self
            .drafts
            .tracked
            .values()
            .map(|record| record.snapshot.clone())
            .collect();
        let mut items = Vec::new();
        for directory in self.draft_paths(&root, true) {
            let session = match self.drafts.take_session(&directory) {
                Ok(Some(session)) => session,
                Ok(None) => continue,
                Err(error) => {
                    self.report_draft_read(&directory, &error);
                    continue;
                }
            };
            for path in self.draft_paths(&session.directory, false) {
                if tracked.contains(&path) {
                    continue;
                }
                match store::read(&path) {
                    Ok(draft) => items.push(Item {
                        path,
                        title: draft
                            .original
                            .as_ref()
                            .map(|path| path.path().display().to_string())
                            .or(draft.label)
                            .unwrap_or_else(|| self.t("drafts.untitled").into()),
                        timestamp: draft.timestamp,
                    }),
                    Err(error) => self.report_draft_read(&path, &error),
                }
            }
        }
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then(a.path.cmp(&b.path)));
        items
    }

    fn draft_paths(&mut self, parent: &Path, directories: bool) -> Vec<PathBuf> {
        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Vec::new(),
            Err(error) => {
                self.report_draft_read(parent, &error);
                return Vec::new();
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            match entry.and_then(|entry| Ok((entry.path(), entry.file_type()?))) {
                Ok((path, kind)) if directories && kind.is_dir() => paths.push(path),
                Ok((path, kind))
                    if !directories
                        && kind.is_file()
                        && path.extension().is_some_and(|ext| ext == "json") =>
                {
                    paths.push(path)
                }
                Ok(_) => {}
                Err(error) => self.report_draft_read(parent, &error),
            }
        }
        paths
    }

    fn report_draft_read(&mut self, path: &Path, error: &io::Error) {
        if self.drafts.inaccessible.insert(path.into()) {
            report("drafts.read", error);
        }
    }

    pub(crate) fn offer_draft_review(&mut self) {
        if self.exit_requested
            || self.quick_input_open()
            || self.close_window_confirm
            || self.gpu.is_none()
            || !self
                .drafts
                .offered_profiles
                .insert(self.profile_root.clone())
        {
            return;
        }
        if !self.draft_items().is_empty() {
            self.show_draft_prompt(
                Review::Offer,
                "drafts.available",
                vec![
                    self.draft_action("review", "drafts.review"),
                    self.draft_action("later", "drafts.later"),
                ],
            );
        }
    }

    pub(crate) fn review_drafts(&mut self) {
        self.cancel_extension_prompt();
        let drafts = self.draft_items();
        if drafts.is_empty() {
            self.status = self.t("drafts.empty").into();
            return;
        }
        let rows = drafts
            .iter()
            .enumerate()
            .map(|(index, draft)| PaletteItem {
                id: index.to_string(),
                title: format!(
                    "{}  {}",
                    draft.title,
                    crate::app::local_history::format_utc(draft.timestamp.saturating_mul(1000))
                ),
                source: "Graphite".into(),
                enabled: true,
            })
            .collect();
        self.show_draft_prompt(Review::List(drafts), "drafts.select", rows);
    }

    fn draft_action(&self, id: &str, key: &str) -> PaletteItem {
        PaletteItem {
            id: id.into(),
            title: self.t(key).into(),
            source: "Graphite".into(),
            enabled: true,
        }
    }

    fn show_draft_prompt(&mut self, review: Review, title: &str, items: Vec<PaletteItem>) {
        self.command_center = None;
        self.quick_open = None;
        self.go_picker = None;
        self.drafts.open_review = Some(review);
        self.command_palette = Some(CommandPalette {
            items,
            extension_prompt: Some((self.t(title).into(), false)),
            ..CommandPalette::default()
        });
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn answer_draft_review(&mut self, option: Option<&str>) -> bool {
        let Some(review) = self.drafts.open_review.take() else {
            return false;
        };
        match (review, option) {
            (Review::Offer, Some("review")) => self.review_drafts(),
            (Review::List(mut items), Some(index)) => {
                if let Some(index) = index
                    .parse::<usize>()
                    .ok()
                    .filter(|index| *index < items.len())
                {
                    self.show_draft_prompt(
                        Review::Choose(items.swap_remove(index)),
                        "drafts.action",
                        vec![
                            self.draft_action("restore", "drafts.restore"),
                            self.draft_action("discard", "drafts.discard"),
                            self.draft_action("later", "drafts.later"),
                        ],
                    );
                }
            }
            (Review::Choose(item), Some("restore")) => self.resolve_draft_item(&item, true),
            (Review::Choose(item), Some("discard")) => self.show_draft_prompt(
                Review::Confirm(item),
                "drafts.confirm",
                vec![
                    self.draft_action("later", "drafts.later"),
                    self.draft_action("discard", "drafts.discard"),
                ],
            ),
            (Review::Confirm(item), Some("discard")) => {
                self.resolve_draft_item(&item, false);
                if !self.draft_items().is_empty() {
                    self.review_drafts();
                }
            }
            _ => {}
        }
        true
    }

    fn resolve_draft_item(&mut self, item: &Item, restore: bool) {
        match self.resolve_draft(&item.path, restore) {
            Ok(()) if !restore => self.status = self.t("drafts.discarded").into(),
            Ok(()) => {}
            Err(error) => {
                report(
                    if restore {
                        "drafts.restore"
                    } else {
                        "drafts.remove"
                    },
                    &error,
                );
                self.status = self.t("drafts.unavailable").into();
            }
        }
    }

    pub(super) fn resolve_draft(&mut self, path: &Path, restore: bool) -> io::Result<()> {
        if self
            .drafts
            .tracked
            .values()
            .any(|entry| entry.snapshot == path)
        {
            return Err(ErrorKind::WouldBlock.into());
        }
        let directory = path.parent().ok_or(ErrorKind::InvalidInput)?;
        if directory.parent() != Some(store::root(&self.profile_root).as_path()) {
            return Err(ErrorKind::InvalidInput.into());
        }
        let session = self
            .drafts
            .take_session(directory)?
            .ok_or(ErrorKind::WouldBlock)?;
        if !restore {
            return store::remove(path);
        }

        let draft = store::read(path)?;
        let original = draft.original.as_ref().map(NativePath::path);
        let mut target = None;
        if let Some(original) = original.as_deref() {
            if let Ok(disk) = file_format::read(original, &self.translator) {
                let draft_bytes = file_format::encode(&draft.text, draft.encoding, draft.ending);
                if disk.fingerprint == Some(file_format::fingerprint(&draft_bytes)) {
                    store::remove(path)?;
                    self.status = self.t("drafts.already_saved").into();
                    return Ok(());
                }
                if draft.base.is_some() && disk.fingerprint == draft.base {
                    // The open path also applies Graphite's configured read-only patterns.
                    self.open_file_as_text(original.into());
                    target = self.documents.id_for_path(original).filter(|id| {
                        let doc = &self.documents[*id];
                        doc.saved_fingerprint == draft.base
                            && !doc.is_dirty()
                            && !doc.buffer.is_read_only()
                            && doc.notebook.is_none()
                            && !self.custom_editor_is_bound(original)
                            && !self.saving.document_pending(*id)
                    });
                }
            }
        }

        if target.is_none() {
            self.new_untitled_document();
            if let Some(label) = original
                .as_deref()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .or(draft.label)
            {
                let id = self.active_document();
                self.documents[id].recovery_label = Some(label);
            }
        }
        let id = self.active_document();
        let end = self.editor().len_chars();
        // Adopt the original file under its session lock before applying the recovered text.
        // A second crash leaves this same checkpoint available.
        self.drafts.tracked.insert(
            id,
            Tracked {
                session,
                snapshot: path.into(),
                written: None,
                running: None,
                next_write: Instant::now(),
                remove_retry: None,
            },
        );
        let result = self
            .editor_mut()
            .apply_transaction(&[graphite_text::TextReplacement {
                range: 0..end,
                text: draft.text,
            }]);
        if result.is_err() {
            self.drafts.tracked.remove(&id);
            return Err(ErrorKind::InvalidData.into());
        }
        self.documents[id].encoding = draft.encoding;
        self.documents[id].line_ending = draft.ending;
        self.documents[id].recovery_pending_save = true;
        self.status = self
            .t(if target.is_none() && original.is_some() {
                "drafts.restored_copy"
            } else {
                "drafts.restored"
            })
            .into();
        Ok(())
    }
}
