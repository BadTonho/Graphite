use super::*;
use std::collections::BTreeSet;

pub(super) struct Bisect {
    original: BTreeSet<String>,
    native_running: BTreeSet<String>,
    vsix_running: bool,
    candidates: Vec<String>,
    stage: Stage,
}

#[derive(Clone, Copy)]
enum Stage {
    NoExtensions,
    FirstHalf,
    SecondHalf,
}

enum Step {
    Trial(BTreeSet<String>),
    Suspect(String),
    Inconclusive,
}

impl Bisect {
    fn answer(&mut self, persists: bool) -> Step {
        if matches!(self.stage, Stage::NoExtensions) {
            return if persists {
                Step::Inconclusive
            } else {
                self.first_half()
            };
        }
        let split = self.candidates.len().div_ceil(2);
        match (self.stage, persists) {
            (Stage::FirstHalf, true) => self.candidates.truncate(split),
            (Stage::FirstHalf, false) if self.candidates.len() > 1 => {
                self.stage = Stage::SecondHalf;
                return Step::Trial(self.candidates[split..].iter().cloned().collect());
            }
            (Stage::SecondHalf, true) => {
                self.candidates.drain(..split);
            }
            _ => return Step::Inconclusive,
        }
        if self.candidates.len() == 1 {
            Step::Suspect(self.candidates[0].clone())
        } else {
            self.first_half()
        }
    }

    fn first_half(&mut self) -> Step {
        self.stage = Stage::FirstHalf;
        Step::Trial(
            self.candidates[..self.candidates.len().div_ceil(2)]
                .iter()
                .cloned()
                .collect(),
        )
    }
}

impl App {
    pub(super) fn start_plugin_bisect(&mut self) {
        let original: BTreeSet<_> = self
            .plugins
            .installed
            .iter()
            .filter(|record| record.enabled)
            .map(|record| record.metadata.id().to_owned())
            .collect();
        if original.is_empty() {
            self.plugins.message = self.t("plugins.none_enabled").into();
            return;
        }
        self.plugins.bisect = Some(Bisect {
            candidates: original.iter().cloned().collect(),
            original,
            native_running: self
                .plugins
                .hosts
                .keys()
                .cloned()
                .chain(self.plugins.startup.iter().cloned())
                .chain(self.plugins.starting.iter().cloned())
                .collect(),
            vsix_running: self.plugins.vsix.is_some(),
            stage: Stage::NoExtensions,
        });
        self.apply_plugin_bisect_trial(&BTreeSet::new());
    }

    pub(super) fn plugin_bisect_answer(&mut self, persists: bool) {
        let Some(bisect) = self.plugins.bisect.as_mut() else {
            return;
        };
        match bisect.answer(persists) {
            Step::Trial(enabled) => self.apply_plugin_bisect_trial(&enabled),
            Step::Suspect(id) => {
                self.cancel_plugin_bisect();
                self.plugins.message = self.tf("plugins.toolbar.bisect_suspect", &[("id", &id)]);
            }
            Step::Inconclusive => {
                self.cancel_plugin_bisect();
                self.plugins.message = self.t("plugins.toolbar.bisect_inconclusive").into();
            }
        }
    }

    pub(super) fn cancel_plugin_bisect(&mut self) {
        if let Some(bisect) = self.plugins.bisect.take() {
            self.apply_plugin_trial(
                &bisect.original,
                &bisect.native_running,
                bisect.vsix_running,
            );
            self.plugins.message.clear();
        }
    }

    fn apply_plugin_bisect_trial(&mut self, enabled: &BTreeSet<String>) {
        let Some(bisect) = &self.plugins.bisect else {
            return;
        };
        let native_running = bisect.native_running.clone();
        self.apply_plugin_trial(enabled, &native_running, bisect.vsix_running);
        self.plugins.message =
            self.tf("plugins.toolbar.bisect_trial", &[("count", &enabled.len())]);
    }

    fn apply_plugin_trial(
        &mut self,
        enabled: &BTreeSet<String>,
        native_running: &BTreeSet<String>,
        vsix_running: bool,
    ) {
        self.stop_workspace_plugins();
        for record in &mut self.plugins.installed {
            record.enabled = enabled.contains(record.metadata.id());
        }
        self.sync_plugin_contributions();
        self.plugins
            .startup
            .extend(native_running.intersection(enabled).cloned());
        if vsix_running
            && self.vsix_workspace_trusted()
            && self.plugins.installed.iter().any(vsix::eligible)
        {
            let runtime = graphite_extension_host::code_oss::Session::bundled_runtime();
            if let Err(error) = self.start_vsix_host(runtime) {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Medium,
                    "plugins.bisect.runtime",
                    &error,
                );
                self.status = error;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tests_both_halves_and_finds_the_reproducing_extension() {
        let mut bisect = Bisect {
            original: BTreeSet::new(),
            native_running: BTreeSet::new(),
            vsix_running: false,
            candidates: ["a", "b", "c"].map(str::to_owned).to_vec(),
            stage: Stage::NoExtensions,
        };
        assert!(matches!(bisect.answer(false), Step::Trial(set) if set.len() == 2));
        assert!(matches!(bisect.answer(false), Step::Trial(set) if set.contains("c")));
        assert!(matches!(bisect.answer(true), Step::Suspect(id) if id == "c"));
    }

    #[test]
    fn reports_inconclusive_when_every_extension_is_disabled() {
        let mut bisect = Bisect {
            original: BTreeSet::new(),
            native_running: BTreeSet::new(),
            vsix_running: false,
            candidates: vec!["a".into()],
            stage: Stage::NoExtensions,
        };
        assert!(matches!(bisect.answer(true), Step::Inconclusive));
    }
}
