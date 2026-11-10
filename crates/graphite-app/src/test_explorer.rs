//! Extension supplied test tree and results independent of the renderer.

use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TestState {
    Discovered,
    Queued,
    Running,
    Passed,
    Failed {
        message: String,
        expected: Option<String>,
        actual: Option<String>,
    },
    Skipped,
    Errored(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TestItem {
    pub id: String,
    pub label: String,
    pub uri: Option<PathBuf>,
    pub line: Option<u32>,
    pub state: TestState,
    pub output: Vec<String>,
    pub children: Vec<TestItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TestController {
    pub id: String,
    pub label: String,
    pub roots: Vec<TestItem>,
    pub run_profiles: Vec<TestRunProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TestRunProfile {
    pub id: u64,
    pub label: String,
    pub kind: TestRunKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestRunKind {
    Run,
    Debug,
    Coverage,
}

#[derive(Default)]
pub(crate) struct TestExplorer {
    pub controllers: BTreeMap<String, TestController>,
    pub active_run: Option<u64>,
    pub selected: Option<String>,
    pub last_failure: Option<(String, String, Option<String>, Option<String>)>,
    next_run: u64,
}

impl TestExplorer {
    pub(crate) fn register_controller(&mut self, id: String, label: String) -> Result<(), String> {
        if id.trim().is_empty() || label.trim().is_empty() {
            return Err("Test controller requires an ID and label".into());
        }
        self.controllers
            .entry(id.clone())
            .or_insert(TestController {
                id,
                label,
                roots: Vec::new(),
                run_profiles: Vec::new(),
            });
        Ok(())
    }

    pub(crate) fn remove_controller(&mut self, id: &str) {
        self.controllers.remove(id);
        if let Some(selected) = &self.selected {
            if selected.starts_with(&format!("{id}/")) {
                self.selected = None;
            }
        }
    }

    pub(crate) fn replace_roots(
        &mut self,
        controller: &str,
        roots: Vec<TestItem>,
    ) -> Result<(), String> {
        let target = self
            .controllers
            .get_mut(controller)
            .ok_or("Test controller is not registered")?;
        target.roots = roots;
        Ok(())
    }

    pub(crate) fn add_profile(
        &mut self,
        controller: &str,
        profile: TestRunProfile,
    ) -> Result<(), String> {
        let target = self
            .controllers
            .get_mut(controller)
            .ok_or("Test controller is not registered")?;
        target
            .run_profiles
            .retain(|existing| existing.id != profile.id);
        target.run_profiles.push(profile);
        Ok(())
    }

    pub(crate) fn begin_run(&mut self) -> Result<u64, String> {
        if self.active_run.is_some() {
            return Err("A test run is already active".into());
        }
        self.next_run = self.next_run.wrapping_add(1).max(1);
        self.active_run = Some(self.next_run);
        Ok(self.next_run)
    }

    pub(crate) fn finish_run(&mut self, id: u64) -> bool {
        if self.active_run == Some(id) {
            self.active_run = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn find_item(&self, id: &str) -> Option<&TestItem> {
        self.controllers
            .values()
            .flat_map(|controller| controller.roots.iter())
            .find_map(|item| find(item, id))
    }

    pub(crate) fn set_state(&mut self, id: &str, state: TestState) -> bool {
        let found = self
            .controllers
            .values_mut()
            .flat_map(|controller| controller.roots.iter_mut())
            .any(|item| set_state(item, id, state.clone()));
        if let TestState::Failed {
            message,
            expected,
            actual,
        } = state
        {
            self.last_failure = Some((id.to_owned(), message, expected, actual));
        }
        found
    }

    pub(crate) fn apply_vscode_diff(&mut self, controller_id: &str, diff: &[serde_json::Value]) {
        for operation in diff {
            let kind = operation["op"].as_u64().unwrap_or(u64::MAX);
            match kind {
                0 => {
                    let item = &operation["item"]["item"];
                    let Some(id) = item["extId"].as_str() else {
                        continue;
                    };
                    let parent = id.rsplit_once('\0').map(|(parent, _)| parent);
                    let uri = item["uri"]["path"].as_str().map(PathBuf::from);
                    let node = TestItem {
                        id: id.to_owned(),
                        label: item["label"].as_str().unwrap_or(id).to_owned(),
                        uri,
                        line: item["range"]["startLineNumber"]
                            .as_u64()
                            .map(|line| line as u32),
                        state: TestState::Discovered,
                        output: Vec::new(),
                        children: Vec::new(),
                    };
                    if let Some(controller) = self.controllers.get_mut(controller_id) {
                        if let Some(parent) = parent {
                            if let Some(parent) = controller
                                .roots
                                .iter_mut()
                                .find_map(|root| find_mut(root, parent))
                            {
                                if !parent.children.iter().any(|child| child.id == node.id) {
                                    parent.children.push(node);
                                }
                            }
                        } else if !controller.roots.iter().any(|root| root.id == node.id) {
                            controller.roots.push(node);
                        }
                    }
                }
                1 => {
                    if let Some(id) = operation["item"]["extId"].as_str() {
                        for controller in self.controllers.values_mut() {
                            for root in &mut controller.roots {
                                if let Some(item) = find_mut(root, id) {
                                    if let Some(label) = operation["item"]["item"]["label"].as_str()
                                    {
                                        item.label = label.to_owned();
                                    }
                                }
                            }
                        }
                    }
                }
                3 | 5 => {
                    if let Some(id) = operation["itemId"].as_str() {
                        if let Some(controller) = self.controllers.get_mut(controller_id) {
                            controller.roots.retain(|root| root.id != id);
                            for root in &mut controller.roots {
                                remove_child(root, id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn profile_for(&self, controller: &str) -> Option<TestRunProfile> {
        self.controllers
            .get(controller)?
            .run_profiles
            .first()
            .cloned()
    }
}

fn find<'a>(item: &'a TestItem, id: &str) -> Option<&'a TestItem> {
    if item.id == id {
        return Some(item);
    }
    item.children.iter().find_map(|child| find(child, id))
}

fn find_mut<'a>(item: &'a mut TestItem, id: &str) -> Option<&'a mut TestItem> {
    if item.id == id {
        return Some(item);
    }
    item.children
        .iter_mut()
        .find_map(|child| find_mut(child, id))
}

fn remove_child(item: &mut TestItem, id: &str) {
    item.children.retain(|child| child.id != id);
    for child in &mut item.children {
        remove_child(child, id);
    }
}

fn set_state(item: &mut TestItem, id: &str, state: TestState) -> bool {
    if item.id == id {
        item.state = state;
        return true;
    }
    item.children
        .iter_mut()
        .any(|child| set_state(child, id, state.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, children: Vec<TestItem>) -> TestItem {
        TestItem {
            id: id.into(),
            label: id.into(),
            uri: None,
            line: None,
            state: TestState::Discovered,
            output: Vec::new(),
            children,
        }
    }

    #[test]
    fn controller_tree_tracks_nested_results_and_failure_details() {
        let mut explorer = TestExplorer::default();
        explorer
            .register_controller("rust".into(), "Rust tests".into())
            .unwrap();
        explorer
            .replace_roots("rust", vec![item("suite", vec![item("case", vec![])])])
            .unwrap();
        assert!(explorer.set_state(
            "case",
            TestState::Failed {
                message: "assertion failed".into(),
                expected: Some("4".into()),
                actual: Some("3".into())
            }
        ));
        assert_eq!(
            explorer.find_item("case").unwrap().state,
            TestState::Failed {
                message: "assertion failed".into(),
                expected: Some("4".into()),
                actual: Some("3".into())
            }
        );
        assert_eq!(
            explorer.last_failure.as_ref().unwrap().2.as_deref(),
            Some("4")
        );
        assert!(!explorer.set_state("missing", TestState::Passed));
    }

    #[test]
    fn runs_are_singleton_and_controller_removal_clears_selection() {
        let mut explorer = TestExplorer::default();
        explorer
            .register_controller("rust".into(), "Rust".into())
            .unwrap();
        explorer.selected = Some("rust/a".into());
        let run = explorer.begin_run().unwrap();
        assert!(explorer.begin_run().is_err());
        assert!(!explorer.finish_run(run + 1));
        assert!(explorer.finish_run(run));
        explorer.remove_controller("rust");
        assert!(explorer.selected.is_none());
    }
}
