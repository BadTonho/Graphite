use super::transport::read_message;
use super::*;
use std::io::Cursor;

#[test]
fn parses_jsonc_launch_profiles_and_defaults() {
    let root = crate::test_support::TempDir::new();
    std::fs::create_dir_all(root.path().join(".vscode")).unwrap();
    std::fs::write(root.path().join(".vscode/launch.json"), r#"{
      // A workspace launch profile
      "version": "0.2.0",
      "configurations": [{"name":"Debug app","type":"example","request":"launch", "adapter":{"command":"adapter"}, "args":["a"]}],
    }"#).unwrap();
    let configs = load_launch_configs(root.path()).unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].name, "Debug app");
    assert_eq!(configs[0].request, RequestKind::Launch);
    assert_eq!(configs[0].args, ["a"]);
    assert!(!configs[0].stop_on_entry);
}

#[test]
fn frames_messages_and_rejects_invalid_or_oversized_lengths() {
    let payload = br#"{"type":"event","seq":1}"#;
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(payload);
    assert_eq!(
        read_message(&mut Cursor::new(bytes)).unwrap().unwrap()["seq"],
        1
    );
    assert!(read_message(&mut Cursor::new(b"Content-Length: nope\r\n\r\n")).is_err());
    let large = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
    assert!(read_message(&mut Cursor::new(large.into_bytes())).is_err());
}

#[derive(Clone, Default)]
struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SharedWriter {
    /// Every request sent so far, oldest first.
    fn requests(&self) -> Vec<Value> {
        let bytes = self.0.lock().unwrap().clone();
        let mut reader = BufReader::new(Cursor::new(bytes));
        let mut requests = Vec::new();
        while let Ok(Some(message)) = read_message(&mut reader) {
            requests.push(message);
        }
        requests
    }

    fn last(&self) -> Value {
        self.requests().pop().expect("a request was sent")
    }
}

fn paused_runtime() -> (DebugRuntime, SharedWriter) {
    let writer = SharedWriter::default();
    let runtime = DebugRuntime {
        session: Some(DapSession::detached(Box::new(writer.clone()))),
        state: DebugState::Paused {
            thread_id: 1,
            reason: "breakpoint".into(),
        },
        ..DebugRuntime::default()
    };
    (runtime, writer)
}

fn respond(runtime: &mut DebugRuntime, request_seq: i64, body: Value) {
    runtime
        .handle_message(
            serde_json::json!({"type":"response","request_seq":request_seq,"success":true,"body":body}),
            Path::new("."),
        )
        .unwrap();
}

fn respond_error(runtime: &mut DebugRuntime, request_seq: i64, message: &str) {
    runtime
        .handle_message(
            serde_json::json!({"type":"response","request_seq":request_seq,"success":false,"message":message}),
            Path::new("."),
        )
        .unwrap();
}

/// Drives stackTrace and scopes so the runtime shows locals `a` and expandable `obj`.
fn paused_with_locals() -> (DebugRuntime, SharedWriter) {
    let (mut runtime, writer) = paused_runtime();
    runtime.request_stack(1).unwrap();
    respond(
        &mut runtime,
        1,
        serde_json::json!({"stackFrames":[
            {"id":10,"name":"main","source":{"path":"a.rs"},"line":3},
            {"id":11,"name":"caller","source":{"path":"b.rs"},"line":9}
        ]}),
    );
    respond(
        &mut runtime,
        2,
        serde_json::json!({"scopes":[
            {"name":"Locals","variablesReference":5},
            {"name":"Globals","variablesReference":6,"expensive":true}
        ]}),
    );
    respond(
        &mut runtime,
        3,
        serde_json::json!({"variables":[
            {"name":"a","value":"1","type":"int","variablesReference":0},
            {"name":"obj","value":"Obj","variablesReference":7}
        ]}),
    );
    (runtime, writer)
}

fn config_with_tasks(pre: Option<&str>, post: Option<&str>) -> LaunchConfig {
    LaunchConfig {
        name: "x".into(),
        adapter_type: "t".into(),
        request: RequestKind::Launch,
        program: None,
        args: Vec::new(),
        cwd: None,
        stop_on_entry: false,
        pre_launch_task: pre.map(str::to_owned),
        post_debug_task: post.map(str::to_owned),
        env: BTreeMap::new(),
        adapter: None,
        server: None,
        pipe: None,
        extra: BTreeMap::new(),
    }
}

#[test]
fn the_selected_configuration_wins_and_the_first_is_the_fallback() {
    let mut runtime = DebugRuntime::default();
    assert!(runtime.launch_config().is_none());
    runtime.launch_configs = vec![
        LaunchConfig {
            name: "first".into(),
            ..config_with_tasks(None, None)
        },
        LaunchConfig {
            name: "second".into(),
            ..config_with_tasks(None, None)
        },
    ];
    assert_eq!(runtime.launch_config().unwrap().name, "first");
    assert!(runtime.select_config("second"));
    assert_eq!(runtime.launch_config().unwrap().name, "second");
    assert!(
        !runtime.select_config("missing"),
        "unknown names are refused"
    );
    assert_eq!(
        runtime.launch_config().unwrap().name,
        "second",
        "choice is kept"
    );
    // The file changed and the selected configuration is gone.
    runtime.launch_configs.remove(1);
    assert_eq!(runtime.launch_config().unwrap().name, "first");
}

#[test]
fn launch_profiles_read_pre_launch_and_post_debug_tasks() {
    let root = crate::test_support::TempDir::new();
    std::fs::create_dir_all(root.path().join(".vscode")).unwrap();
    std::fs::write(
        root.path().join(".vscode/launch.json"),
        r#"{"configurations":[
            {"name":"a","type":"t","request":"launch","preLaunchTask":"build","postDebugTask":"clean"},
            {"name":"b","type":"t","request":"launch"}
        ]}"#,
    )
    .unwrap();
    let configs = load_launch_configs(root.path()).unwrap();
    assert_eq!(configs[0].pre_launch_task.as_deref(), Some("build"));
    assert_eq!(configs[0].post_debug_task.as_deref(), Some("clean"));
    assert_eq!(configs[1].pre_launch_task, None);
    assert!(
        !configs[0].extra.contains_key("preLaunchTask"),
        "task fields are modelled, not passed through"
    );
}

#[test]
fn the_adapter_never_sees_graphite_side_launch_settings() {
    let (mut runtime, writer) = paused_runtime();
    runtime.initialize_seq = Some(1);
    runtime.active_config = Some(config_with_tasks(Some("build"), Some("clean")));
    respond(&mut runtime, 1, serde_json::json!({}));
    let launch = writer.last();
    assert_eq!(launch["command"], "launch");
    let arguments = launch["arguments"].as_object().unwrap();
    for key in ["preLaunchTask", "postDebugTask", "adapter"] {
        assert!(!arguments.contains_key(key), "{key} leaked to the adapter");
    }
    assert_eq!(arguments["name"], "x");
}

#[test]
fn the_post_debug_task_is_ready_once_when_the_session_is_stopped() {
    let (mut runtime, _writer) = paused_runtime();
    runtime.post_debug_task = Some("clean".into());
    assert_eq!(runtime.take_post_debug_task(), None, "not while running");
    runtime.stop().unwrap();
    assert_eq!(runtime.take_post_debug_task().as_deref(), Some("clean"));
    assert_eq!(runtime.take_post_debug_task(), None, "handed out once");
    runtime.stop().unwrap();
    assert_eq!(
        runtime.take_post_debug_task(),
        None,
        "a second stop adds nothing"
    );
}

#[test]
fn the_post_debug_task_is_ready_when_the_program_terminates() {
    let (mut runtime, _writer) = paused_runtime();
    runtime.post_debug_task = Some("clean".into());
    runtime
        .handle_message(
            serde_json::json!({"type":"event","event":"terminated"}),
            Path::new("."),
        )
        .unwrap();
    assert_eq!(runtime.take_post_debug_task().as_deref(), Some("clean"));
}

#[test]
fn the_post_debug_task_is_ready_when_the_adapter_connection_closes() {
    // A detached session's channel has no sender, so polling sees the adapter close.
    let (mut runtime, _writer) = paused_runtime();
    runtime.post_debug_task = Some("clean".into());
    runtime.poll(Path::new(".")).unwrap();
    assert!(runtime.session.is_none());
    assert_eq!(runtime.take_post_debug_task().as_deref(), Some("clean"));
}

#[test]
fn sessions_without_a_post_debug_task_hand_out_nothing() {
    let (mut runtime, _writer) = paused_runtime();
    runtime.stop().unwrap();
    assert_eq!(runtime.take_post_debug_task(), None);
}

fn context() -> VariableContext {
    let workspace = std::env::temp_dir().join("proj");
    VariableContext {
        active_file: Some(workspace.join("src").join("main.py")),
        workspace,
    }
}

#[test]
fn launch_variables_expand_workspace_and_file_parts() {
    let context = context();
    let workspace = context.workspace.display().to_string();
    let sep = std::path::MAIN_SEPARATOR;
    let expand = |text: &str| expand_variables(text, &context);
    assert_eq!(expand("${workspaceFolder}"), workspace);
    assert_eq!(expand("${workspaceFolderBasename}"), "proj");
    assert_eq!(expand("${cwd}"), workspace);
    assert_eq!(expand("${fileBasename}"), "main.py");
    assert_eq!(expand("${fileBasenameNoExtension}"), "main");
    assert_eq!(expand("${fileExtname}"), ".py");
    assert_eq!(expand("${relativeFile}"), format!("src{sep}main.py"));
    assert_eq!(expand("${relativeFileDirname}"), "src");
    assert_eq!(expand("${fileDirname}"), format!("{workspace}{sep}src"));
    assert_eq!(
        expand("${file}"),
        format!("{workspace}{sep}src{sep}main.py")
    );
    assert_eq!(expand("a${pathSeparator}b${/}c"), format!("a{sep}b{sep}c"));
    assert_eq!(
        expand("${workspaceFolder}/${fileBasename}"),
        format!("{workspace}/main.py"),
        "several variables in one string"
    );
}

#[test]
fn environment_variables_expand_and_missing_ones_become_empty() {
    let context = context();
    std::env::set_var("GRAPHITE_DEBUG_TEST_VALUE", "from-env");
    assert_eq!(
        expand_variables("${env:GRAPHITE_DEBUG_TEST_VALUE}!", &context),
        "from-env!"
    );
    assert_eq!(
        expand_variables("[${env:GRAPHITE_DEBUG_TEST_UNSET_VALUE}]", &context),
        "[]"
    );
}

#[test]
fn unknown_or_unavailable_variables_stay_visible() {
    let mut context = context();
    context.active_file = None;
    for text in [
        "${unknown}",
        "${file}",
        "${relativeFile}",
        "${input:name}",
        "${command:pick}",
        "${unterminated",
        "$notavariable",
        "${}",
    ] {
        assert_eq!(expand_variables(text, &context), text);
    }
    assert_eq!(expand_variables("", &context), "");
    assert_eq!(
        expand_variables("é${file}ü${workspaceFolderBasename}", &context),
        "é${file}üproj",
        "multi-byte text around variables is kept"
    );
}

#[test]
fn expansion_reaches_nested_launch_values() {
    let context = context();
    let mut value = serde_json::json!({
        "args": ["--root", "${workspaceFolder}", 3],
        "env": {"A": "${fileBasename}"},
        "n": 1, "flag": true, "nothing": null
    });
    expand_value(&mut value, &context);
    assert_eq!(value["args"][1], context.workspace.display().to_string());
    assert_eq!(value["args"][2], 3);
    assert_eq!(value["env"]["A"], "main.py");
    assert_eq!(value["flag"], true);
}

#[test]
fn launch_arguments_get_expanded_absolute_program_and_cwd() {
    let (mut runtime, writer) = paused_runtime();
    runtime.initialize_seq = Some(1);
    runtime.active_file = Some(PathBuf::from("/w/app.py"));
    let mut config = config_with_tasks(None, None);
    config.program = Some(PathBuf::from("${workspaceFolder}/main.py"));
    config.args = vec!["--file".into(), "${fileBasename}".into()];
    config
        .env
        .insert("ROOT".into(), "${workspaceFolder}".into());
    runtime.active_config = Some(config);
    let workspace = std::env::temp_dir().join("proj");
    runtime
        .handle_message(
            serde_json::json!({"type":"response","request_seq":1,"success":true,"body":{}}),
            &workspace,
        )
        .unwrap();
    let launch = writer.last();
    let arguments = &launch["arguments"];
    let root = workspace.display().to_string();
    assert_eq!(arguments["program"], format!("{root}/main.py"));
    assert_eq!(arguments["args"][1], "app.py");
    assert_eq!(arguments["env"]["ROOT"], root);
    assert_eq!(arguments["cwd"], root, "defaults to the workspace");
}

#[test]
fn a_relative_program_and_cwd_resolve_against_the_workspace() {
    let (mut runtime, writer) = paused_runtime();
    runtime.initialize_seq = Some(1);
    let mut config = config_with_tasks(None, None);
    config.program = Some(PathBuf::from("bin/run"));
    config.cwd = Some(PathBuf::from("sub"));
    runtime.active_config = Some(config);
    let workspace = std::env::temp_dir().join("proj");
    runtime
        .handle_message(
            serde_json::json!({"type":"response","request_seq":1,"success":true,"body":{}}),
            &workspace,
        )
        .unwrap();
    let arguments = &writer.last()["arguments"];
    assert_eq!(
        arguments["program"],
        workspace.join("bin/run").display().to_string()
    );
    assert_eq!(
        arguments["cwd"],
        workspace.join("sub").display().to_string()
    );
}

#[test]
fn a_stop_loads_the_first_cheap_scope_and_lists_its_variables() {
    let (runtime, writer) = paused_with_locals();
    let requests = writer.requests();
    assert_eq!(requests[1]["command"], "scopes");
    assert_eq!(requests[1]["arguments"]["frameId"], 10);
    assert_eq!(requests[2]["command"], "variables");
    assert_eq!(requests[2]["arguments"]["variablesReference"], 5);
    let rows = runtime.variable_rows();
    let labels: Vec<_> = rows
        .iter()
        .map(|row| (row.depth, row.name.as_str(), row.expanded))
        .collect();
    assert_eq!(
        labels,
        [
            (0, "Locals", true),
            (1, "a", false),
            (1, "obj", false),
            (0, "Globals", false)
        ]
    );
    assert_eq!(rows[1].value, "1");
    assert!(rows[0].is_scope && !rows[1].is_scope);
}

#[test]
fn expanding_a_variable_fetches_children_once_and_collapsing_hides_them() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.toggle_variable(7).unwrap();
    assert_eq!(writer.last()["arguments"]["variablesReference"], 7);
    respond(
        &mut runtime,
        4,
        serde_json::json!({"variables":[{"name":"field","value":"2","variablesReference":0}]}),
    );
    let rows = runtime.variable_rows();
    assert_eq!(rows[3].name, "field");
    assert_eq!(rows[3].depth, 2);
    assert!(rows[2].expanded);

    runtime.toggle_variable(7).unwrap();
    assert_eq!(runtime.variable_rows().len(), 4);
    let sent = writer.requests().len();
    runtime.toggle_variable(7).unwrap();
    assert_eq!(
        writer.requests().len(),
        sent,
        "cached children are not fetched again"
    );
    assert_eq!(runtime.variable_rows().len(), 5);
}

#[test]
fn self_referencing_variables_do_not_loop_forever() {
    let (mut runtime, _writer) = paused_with_locals();
    runtime.toggle_variable(7).unwrap();
    respond(
        &mut runtime,
        4,
        serde_json::json!({"variables":[{"name":"me","value":"Obj","variablesReference":7}]}),
    );
    let rows = runtime.variable_rows();
    assert!(rows.len() < 10, "cycle was cut: {} rows", rows.len());
}

#[test]
fn a_failed_variables_request_reports_and_collapses() {
    let (mut runtime, _writer) = paused_with_locals();
    runtime.toggle_variable(7).unwrap();
    respond_error(&mut runtime, 4, "cannot read");
    assert!(!runtime.variable_rows()[2].expanded);
    assert_eq!(runtime.output.last().unwrap().text, "cannot read");
}

#[test]
fn selecting_another_frame_reloads_scopes_and_watches() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.add_watch("a + 1").unwrap();
    runtime.select_frame(1).unwrap();
    assert_eq!(runtime.selected_frame_id(), Some(11));
    assert!(runtime.variable_rows().is_empty(), "old locals are dropped");
    let requests = writer.requests();
    let scopes = requests
        .iter()
        .rev()
        .find(|request| request["command"] == "scopes")
        .unwrap();
    assert_eq!(scopes["arguments"]["frameId"], 11);
    assert_eq!(writer.last()["command"], "evaluate");
    assert_eq!(writer.last()["arguments"]["frameId"], 11);
    // Out-of-range indexes are ignored instead of panicking.
    runtime.select_frame(99).unwrap();
    assert_eq!(runtime.selected_frame_id(), Some(11));
}

#[test]
fn watches_show_results_errors_and_survive_frame_changes() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.add_watch("a + 1").unwrap();
    runtime.add_watch("a + 1").unwrap();
    runtime.add_watch("  ").unwrap();
    assert_eq!(
        runtime.watches.len(),
        1,
        "duplicates and blanks are ignored"
    );
    let request = writer.last();
    assert_eq!(request["arguments"]["context"], "watch");
    assert_eq!(request["arguments"]["frameId"], 10);
    respond(&mut runtime, 4, serde_json::json!({"result":"2"}));
    assert_eq!(runtime.watches[0].result, Some(Ok("2".into())));

    runtime.add_watch("missing").unwrap();
    respond_error(&mut runtime, 5, "not defined");
    assert_eq!(runtime.watches[1].result, Some(Err("not defined".into())));
    assert!(
        runtime.output.iter().all(|line| line.text != "not defined"),
        "watch errors stay in the watch row"
    );

    runtime.remove_watch(0);
    assert_eq!(runtime.watches[0].expression, "missing");
}

#[test]
fn repl_input_is_echoed_and_its_result_lands_in_the_console() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.evaluate_repl("a").unwrap();
    assert_eq!(writer.last()["arguments"]["context"], "repl");
    respond(&mut runtime, 4, serde_json::json!({"result":"1"}));
    let lines: Vec<_> = runtime
        .output
        .iter()
        .map(|line| (line.category.as_str(), line.text.as_str()))
        .collect();
    assert_eq!(lines, [("console", "> a"), ("console", "1")]);

    runtime.evaluate_repl("nope").unwrap();
    respond_error(&mut runtime, 5, "undefined: nope");
    assert_eq!(
        runtime.output.last().map(|line| line.category.as_str()),
        Some("stderr")
    );
    assert!(runtime.watches.is_empty(), "repl input is not a watch");
}

#[test]
fn exception_filters_come_from_the_adapter_and_are_sent_when_toggled() {
    let (mut runtime, writer) = paused_runtime();
    runtime.initialize_seq = Some(1);
    runtime.active_config = Some(config_with_tasks(None, None));
    respond(
        &mut runtime,
        1,
        serde_json::json!({"exceptionBreakpointFilters":[
            {"filter":"raised","label":"Raised Exceptions","default":false},
            {"filter":"uncaught","label":"Uncaught Exceptions","default":true}
        ]}),
    );
    let enabled: Vec<_> = runtime
        .exception_filters
        .iter()
        .map(|filter| (filter.label.as_str(), filter.enabled))
        .collect();
    assert_eq!(
        enabled,
        [("Raised Exceptions", false), ("Uncaught Exceptions", true)]
    );

    runtime.toggle_exception_filter(0).unwrap();
    let request = writer.last();
    assert_eq!(request["command"], "setExceptionBreakpoints");
    assert_eq!(
        request["arguments"]["filters"],
        serde_json::json!(["raised", "uncaught"])
    );
    runtime.toggle_exception_filter(1).unwrap();
    assert_eq!(
        writer.last()["arguments"]["filters"],
        serde_json::json!(["raised"])
    );

    // The choices survive a restart that advertises the same filters.
    runtime.adopt_exception_filters(&serde_json::json!({"exceptionBreakpointFilters":[
        {"filter":"raised","label":"Raised","default":false},
        {"filter":"uncaught","label":"Uncaught","default":true}
    ]}));
    assert!(runtime.exception_filters[0].enabled);
    assert!(!runtime.exception_filters[1].enabled);
}

#[test]
fn the_initialized_event_sends_exception_filters_before_configuration_done() {
    let (mut runtime, writer) = paused_runtime();
    runtime.exception_filters = vec![ExceptionFilter {
        id: "all".into(),
        label: "All".into(),
        enabled: true,
    }];
    runtime
        .handle_message(
            serde_json::json!({"type":"event","event":"initialized"}),
            Path::new("."),
        )
        .unwrap();
    let commands: Vec<_> = writer
        .requests()
        .iter()
        .map(|request| request["command"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(commands, ["setExceptionBreakpoints", "configurationDone"]);
}

#[test]
fn adapters_without_exception_filters_get_no_exception_request() {
    let (mut runtime, writer) = paused_runtime();
    runtime.toggle_exception_filter(0).unwrap();
    runtime
        .handle_message(
            serde_json::json!({"type":"event","event":"initialized"}),
            Path::new("."),
        )
        .unwrap();
    let commands: Vec<_> = writer
        .requests()
        .iter()
        .map(|request| request["command"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(commands, ["configurationDone"]);
}

#[test]
fn stepping_sends_the_paused_thread_and_clears_the_stale_stop() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.step_over().unwrap();
    let request = writer.last();
    assert_eq!(request["command"], "next");
    assert_eq!(request["arguments"]["threadId"], 1);
    assert_eq!(runtime.state, DebugState::Running);
    assert!(runtime.frames.is_empty());
    assert!(runtime.variable_rows().is_empty());
    assert!(runtime.step_over().is_err(), "cannot step while running");
}

#[test]
fn serializes_requests_using_content_length() {
    let mut bytes = Vec::new();
    let payload = serde_json::to_vec(
        &serde_json::json!({"seq":1,"type":"request","command":"continue","arguments":{}}),
    )
    .unwrap();
    write!(bytes, "Content-Length: {}\r\n\r\n", payload.len()).unwrap();
    bytes.extend_from_slice(&payload);
    let mut reader = BufReader::new(Cursor::new(bytes));
    assert_eq!(
        read_message(&mut reader).unwrap().unwrap()["command"],
        "continue"
    );
}

#[test]
fn a_disabled_breakpoint_is_kept_but_not_sent_to_the_adapter() {
    let (mut runtime, writer) = paused_runtime();
    let file = PathBuf::from("a.rs");
    runtime
        .set_breakpoints(
            &file,
            vec![
                serde_json::json!({"line": 3}),
                serde_json::json!({"line": 5, "graphiteDisabled": true}),
                serde_json::json!({"line": 8, "condition": "x > 1"}),
            ],
        )
        .unwrap();
    let sent = writer.last();
    assert_eq!(sent["command"], "setBreakpoints");
    assert_eq!(
        sent["arguments"]["breakpoints"],
        serde_json::json!([{"line": 3}, {"line": 8, "condition": "x > 1"}])
    );
    assert_eq!(
        runtime.breakpoints[&file].len(),
        3,
        "the list keeps the disabled one"
    );
    assert!(is_disabled(&runtime.breakpoints[&file][1]));
}

#[test]
fn function_breakpoints_are_sent_deduplicated_and_cleared() {
    let (mut runtime, writer) = paused_runtime();
    runtime.add_function_breakpoint("  main ").unwrap();
    runtime.add_function_breakpoint("main").unwrap();
    runtime.add_function_breakpoint("   ").unwrap();
    runtime.add_function_breakpoint("helper").unwrap();
    let requests = writer.requests();
    assert_eq!(requests.len(), 2, "only the two new names were sent");
    assert_eq!(requests[1]["command"], "setFunctionBreakpoints");
    assert_eq!(
        requests[1]["arguments"]["breakpoints"],
        serde_json::json!([{"name": "main"}, {"name": "helper"}])
    );
    runtime.remove_function_breakpoints().unwrap();
    assert_eq!(
        writer.last()["arguments"]["breakpoints"],
        serde_json::json!([])
    );
    assert!(runtime.function_breakpoints.is_empty());
}

#[test]
fn function_breakpoints_are_held_back_while_all_breakpoints_are_off() {
    let (mut runtime, writer) = paused_runtime();
    runtime.breakpoints_disabled = true;
    runtime.add_function_breakpoint("main").unwrap();
    assert_eq!(
        writer.last()["arguments"]["breakpoints"],
        serde_json::json!([])
    );
    assert_eq!(runtime.function_breakpoints, ["main"], "still remembered");
}

#[test]
fn data_breakpoints_are_sent_deduplicated_and_respect_disabled_flag() {
    let (mut runtime, writer) = paused_runtime();
    runtime
        .add_data_breakpoint("0x1000", Some("write"), Some("x > 0"))
        .unwrap();
    runtime
        .add_data_breakpoint("0x1000", Some("readWrite"), None)
        .unwrap();
    assert_eq!(runtime.data_breakpoints.len(), 1);
    let req = writer.last();
    assert_eq!(req["command"], "setDataBreakpoints");
    assert_eq!(req["arguments"]["breakpoints"][0]["dataId"], "0x1000");
    assert_eq!(
        req["arguments"]["breakpoints"][0]["accessType"],
        "readWrite"
    );

    runtime.breakpoints_disabled = true;
    runtime.send_data_breakpoints().unwrap();
    assert_eq!(
        writer.last()["arguments"]["breakpoints"],
        serde_json::json!([])
    );

    runtime.remove_data_breakpoints().unwrap();
    assert!(runtime.data_breakpoints.is_empty());
}

#[test]
fn setting_a_variable_sends_the_request_and_shows_the_new_value() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.set_variable("a", "42").unwrap();
    let request = writer.last();
    assert_eq!(request["command"], "setVariable");
    assert_eq!(request["arguments"]["variablesReference"], 5);
    assert_eq!(request["arguments"]["name"], "a");
    assert_eq!(request["arguments"]["value"], "42");
    let seq = request["seq"].as_i64().unwrap();
    respond(
        &mut runtime,
        seq,
        serde_json::json!({"value": "42", "type": "int"}),
    );
    let row = runtime
        .variable_rows()
        .into_iter()
        .find(|row| row.name == "a")
        .unwrap();
    assert_eq!(row.value, "42");
}

#[test]
fn a_rejected_set_variable_reports_the_error_and_keeps_the_value() {
    let (mut runtime, writer) = paused_with_locals();
    runtime.set_variable("a", "oops").unwrap();
    let seq = writer.last()["seq"].as_i64().unwrap();
    respond_error(&mut runtime, seq, "not a number");
    assert_eq!(runtime.output.last().unwrap().text, "not a number");
    let row = runtime
        .variable_rows()
        .into_iter()
        .find(|row| row.name == "a")
        .unwrap();
    assert_eq!(row.value, "1");
}

#[test]
fn setting_an_unknown_variable_is_refused_without_a_request() {
    let (mut runtime, writer) = paused_with_locals();
    let before = writer.requests().len();
    assert!(runtime.set_variable("nope", "1").is_err());
    assert_eq!(writer.requests().len(), before);
}
