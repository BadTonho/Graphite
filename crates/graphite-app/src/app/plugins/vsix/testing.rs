use super::*;
use crate::test_explorer::{TestRunKind, TestRunProfile, TestState};

const TEST_CALL_OFFSET: u64 = 4_000_000;

impl App {
    pub(crate) fn run_selected_test(&mut self) -> std::result::Result<(), String> {
        if !self
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()))
        {
            return Err("Workspace is restricted; trust it before running tests".into());
        }
        let id = self
            .test_explorer
            .selected
            .clone()
            .ok_or("Select a test first")?;
        let Some(runtime) = self.plugins.vsix.as_mut().filter(|runtime| runtime.ready) else {
            return Err("Start a trusted VSIX test provider first".into());
        };
        let (controller_id, profile) = self
            .test_explorer
            .controllers
            .iter()
            .find_map(|(controller_id, controller)| {
                controller
                    .roots
                    .iter()
                    .any(|root| find_item(root, &id).is_some())
                    .then(|| {
                        (
                            controller_id.clone(),
                            controller.run_profiles.first().cloned(),
                        )
                    })
            })
            .ok_or("Selected test is no longer available")?;
        let profile = profile.ok_or("The test provider has no run profile")?;
        let run = self.test_explorer.begin_run()?;
        let call_id = TEST_CALL_OFFSET + run;
        runtime.test_calls.insert(call_id);
        let request = json!({
            "controllerId":controller_id,
            "profileId":profile.id,
            "testIds":[id],
            "excludeExtIds":[],
            "runId":format!("graphite-{run}"),
        });
        if let Err(error) = runtime.session.send(json!({
            "type":"call","id":call_id,"actor":"ExtHostTesting",
            "method":"$runControllerTests","args":[[request]],"cancellable":true,
        })) {
            runtime.test_calls.remove(&call_id);
            self.test_explorer.finish_run(run);
            return Err(error.to_string());
        }
        Ok(())
    }

    pub(crate) fn cancel_test_run(&mut self) {
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            return;
        };
        let Some(run) = self.test_explorer.active_run else {
            return;
        };
        let call_id = TEST_CALL_OFFSET + run;
        runtime.test_calls.remove(&call_id);
        let _ = runtime
            .session
            .send(json!({"type":"cancelCall","id":call_id}));
        self.test_explorer.finish_run(run);
    }
}

fn find_item<'a>(
    item: &'a crate::test_explorer::TestItem,
    id: &str,
) -> Option<&'a crate::test_explorer::TestItem> {
    if item.id == id {
        return Some(item);
    }
    item.children.iter().find_map(|child| find_item(child, id))
}

pub(super) fn dispatch(
    app: &mut App,
    _runtime: &mut Runtime,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    if actor != "MainThreadTesting" {
        return None;
    }
    if !matches!(
        method,
        "$registerTestController"
            | "$unregisterTestController"
            | "$updateController"
            | "$publishTestRunProfile"
            | "$removeTestProfile"
            | "$publishDiff"
            | "$subscribeToDiffs"
            | "$unsubscribeFromDiffs"
            | "$markTestRetired"
            | "$setDefaultRunProfiles"
            | "$updateTestStateInRun"
            | "$appendTestMessagesInRun"
            | "$appendOutputToRun"
            | "$startedExtensionTestRun"
            | "$startedTestRunTask"
            | "$finishedTestRunTask"
            | "$finishedExtensionTestRun"
    ) {
        return None;
    }
    let result = (|| -> std::result::Result<Value, String> {
        let text = |index: usize| {
            args.get(index)
                .and_then(Value::as_str)
                .ok_or("Invalid VSIX test string argument")
        };
        let result = match method {
            "$registerTestController" => app
                .test_explorer
                .register_controller(text(0)?.to_owned(), text(1)?.to_owned())
                .map(|_| Value::Null),
            "$unregisterTestController" => {
                if let Some(id) = args.first().and_then(Value::as_str) {
                    app.test_explorer.remove_controller(id);
                }
                Ok(Value::Null)
            }
            "$updateController" => {
                if let (Some(id), Some(label)) = (
                    args.first().and_then(Value::as_str),
                    args.get(1).and_then(|value| value["label"].as_str()),
                ) {
                    if let Some(controller) = app.test_explorer.controllers.get_mut(id) {
                        controller.label = label.to_owned();
                    }
                }
                Ok(Value::Null)
            }
            "$publishTestRunProfile" => {
                let profile = args.first().ok_or("Missing test run profile");
                match profile {
                    Ok(profile) => {
                        let kind = match profile["group"].as_u64().unwrap_or(2) {
                            4 => TestRunKind::Debug,
                            8 => TestRunKind::Coverage,
                            _ => TestRunKind::Run,
                        };
                        let controller_id = profile["controllerId"]
                            .as_str()
                            .ok_or("Invalid test controller ID")?;
                        app.test_explorer
                            .add_profile(
                                controller_id,
                                TestRunProfile {
                                    id: profile["profileId"].as_u64().unwrap_or_default(),
                                    label: profile["label"].as_str().unwrap_or("Run").to_owned(),
                                    kind,
                                },
                            )
                            .map(|_| Value::Null)
                    }
                    Err(error) => Err(error.to_owned()),
                }
            }
            "$removeTestProfile" => {
                if let (Some(id), Some(profile)) = (
                    args.first().and_then(Value::as_str),
                    args.get(1).and_then(Value::as_u64),
                ) {
                    if let Some(controller) = app.test_explorer.controllers.get_mut(id) {
                        controller.run_profiles.retain(|item| item.id != profile);
                    }
                }
                Ok(Value::Null)
            }
            "$publishDiff" => {
                let id = text(0)?;
                let diff = args
                    .get(1)
                    .and_then(Value::as_array)
                    .ok_or("Invalid test diff")?;
                app.test_explorer.apply_vscode_diff(id, diff);
                Ok(Value::Null)
            }
            "$subscribeToDiffs"
            | "$unsubscribeFromDiffs"
            | "$markTestRetired"
            | "$setDefaultRunProfiles" => Ok(Value::Null),
            "$updateTestStateInRun" => {
                let id = text(2)?;
                let state = match args.get(3).and_then(Value::as_u64).unwrap_or(0) {
                    1 => TestState::Queued,
                    2 => TestState::Running,
                    3 => TestState::Passed,
                    4 => TestState::Failed {
                        message: "Test failed".into(),
                        expected: None,
                        actual: None,
                    },
                    5 => TestState::Skipped,
                    6 => TestState::Errored("Test errored".into()),
                    _ => TestState::Discovered,
                };
                app.test_explorer.set_state(id, state);
                Ok(Value::Null)
            }
            "$appendTestMessagesInRun" => {
                let id = text(2)?;
                let messages = args
                    .get(3)
                    .and_then(Value::as_array)
                    .ok_or("Invalid test messages")?;
                if let Some(item) = app
                    .test_explorer
                    .controllers
                    .values_mut()
                    .flat_map(|controller| controller.roots.iter_mut())
                    .find_map(|root| find_test_mut(root, id))
                {
                    for message in messages {
                        item.output.push(
                            message["message"]
                                .as_str()
                                .or_else(|| message["message"]["value"].as_str())
                                .unwrap_or_default()
                                .to_owned(),
                        );
                        if message["type"].as_u64() == Some(0) {
                            let error = message["message"]
                                .as_str()
                                .or_else(|| message["message"]["value"].as_str())
                                .unwrap_or("Test failed")
                                .to_owned();
                            item.state = TestState::Failed {
                                message: error,
                                expected: message["expected"].as_str().map(str::to_owned),
                                actual: message["actual"].as_str().map(str::to_owned),
                            };
                        }
                    }
                }
                Ok(Value::Null)
            }
            "$appendOutputToRun" => {
                let output = args
                    .get(2)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if let Some(id) = args.get(4).and_then(Value::as_str) {
                    if let Some(item) = app
                        .test_explorer
                        .controllers
                        .values_mut()
                        .flat_map(|controller| controller.roots.iter_mut())
                        .find_map(|root| find_test_mut(root, id))
                    {
                        item.output.push(output);
                    }
                }
                Ok(Value::Null)
            }
            "$startedExtensionTestRun"
            | "$startedTestRunTask"
            | "$finishedTestRunTask"
            | "$finishedExtensionTestRun" => {
                if method == "$finishedExtensionTestRun" {
                    if let Some(run) = app.test_explorer.active_run {
                        app.test_explorer.finish_run(run);
                    }
                }
                Ok(Value::Null)
            }
            _ => unreachable!(),
        };
        result
    })();
    Some(result)
}

fn find_test_mut<'a>(
    item: &'a mut crate::test_explorer::TestItem,
    id: &str,
) -> Option<&'a mut crate::test_explorer::TestItem> {
    if item.id == id {
        return Some(item);
    }
    item.children
        .iter_mut()
        .find_map(|child| find_test_mut(child, id))
}
