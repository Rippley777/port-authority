use super::*;
use crate::{autopilot::capture::Capture, ports::scanner::Scanner};
use std::{
    collections::BTreeMap,
    fs,
    net::TcpListener,
    process::{Child, Stdio},
    thread,
};
#[test]
#[ignore = "detached supervisor fixture"]
fn supervisor_fixture() {
    std::process::exit(executor::supervise());
}
#[test]
#[ignore = "listener fixture"]
fn listener_fixture() {
    assert_eq!(
        std::env::current_dir().unwrap().to_string_lossy(),
        std::env::var("PA_EXPECT_CWD").unwrap()
    );
    assert_eq!(
        std::env::var("API_TOKEN").unwrap(),
        "fixture-secret-do-not-expose"
    );
    let ports: Vec<u16> = std::env::var("PA_PORTS")
        .unwrap()
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect();
    let _listeners: Vec<_> = ports
        .iter()
        .map(|p| TcpListener::bind(("127.0.0.1", *p)).unwrap())
        .collect();
    println!("Ready: fixture-secret-do-not-expose");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
struct Fixture {
    path: PathBuf,
    children: Vec<Child>,
    roots: Vec<ProcessIdentity>,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pa-recovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        let path = path.canonicalize().unwrap();
        fs::write(path.join("package.json"), r#"{"name":"recovery-fixture"}"#).unwrap();
        Self {
            path,
            children: vec![],
            roots: vec![],
        }
    }
    fn capture(&self, ports: &[u16]) -> Capture {
        let executable = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut env: BTreeMap<String, String> = std::env::vars().collect();
        env.insert("PA_EXPECT_CWD".into(), self.path.to_string_lossy().into());
        env.insert(
            "PA_PORTS".into(),
            ports
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        env.insert("API_TOKEN".into(), "fixture-secret-do-not-expose".into());
        Capture {
            argv: vec![
                executable.clone(),
                "--ignored".into(),
                "--exact".into(),
                "recovery::tests::listener_fixture".into(),
                "--nocapture".into(),
            ],
            executable,
            cwd: self.path.to_string_lossy().into(),
            env,
            error: String::new(),
            exit_code: 1,
        }
    }
    fn start(&mut self, c: &Capture) -> PortEntry {
        let child = c
            .command()
            .unwrap()
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        self.children.push(child);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(entry) = Scanner::new()
                .scan()
                .unwrap()
                .into_iter()
                .find(|p| p.pid == Some(pid))
            {
                return entry;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(30));
        }
    }
    fn engine(&self) -> Recovery {
        Recovery::new(
            &self.path.join("timeline.sqlite"),
            Arc::new(Mutex::new(Scanner::new())),
            ProjectEngine::new(None),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for root in &self.roots {
            if let Ok(tree) = ancestry::inspect_tree(*root) {
                let _ = safety::stop(&tree, true);
            }
        }
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn port() -> u16 {
    for _ in 0..200 {
        let port = 10000 + (uuid::Uuid::new_v4().as_u128() % 20000) as u16;
        if port_free(port, &[]) {
            return port;
        }
    }
    panic!("No available fixture port")
}
#[test]
fn command_package_manager_cwd_and_shell_syntax_are_preserved() {
    let f = Fixture::new();
    let mut capture = f.capture(&[]);
    for argv in [
        vec!["cargo", "run"],
        vec!["npm", "run", "dev"],
        vec!["pnpm", "dev"],
        vec!["yarn", "dev"],
        vec!["bun", "dev"],
        vec!["python3", "-m", "uvicorn", "app:app"],
    ] {
        capture.argv = argv.iter().map(|s| s.to_string()).collect();
        capture.env.clear();
        let context = launch_context::from_capture(
            &capture,
            ProcessIdentity {
                pid: 20,
                started_at: 10,
            },
            Source::ShellObserved,
        );
        assert_eq!(context.command, argv.join(" "));
        assert_eq!(context.working_directory, capture.cwd);
        assert!(adapters::supported(&capture.argv));
    }
    capture.executable = "/bin/sh".into();
    capture.argv = vec![
        "sh".into(),
        "-c".into(),
        "source .venv/bin/activate && python app.py | tee dev.log".into(),
    ];
    let context = launch_context::from_capture(
        &capture,
        ProcessIdentity {
            pid: 20,
            started_at: 10,
        },
        Source::ShellObserved,
    );
    assert_eq!(context.kind, LaunchKind::ShellCommand);
    assert_eq!(context.args[1], capture.argv[2]);
    assert!(!adapters::supported(&["npm run dev".into()]));
    assert!(!adapters::supported(&["node".into(), "vite.js".into()]));
    assert!(adapters::excluded("docker-proxy"));
}
#[test]
fn secrets_never_enter_metadata_or_persistence() {
    let f = Fixture::new();
    let mut c = f.capture(&[]);
    c.argv = vec![
        "npm".into(),
        "--token".into(),
        "argv-secret".into(),
        "--password=inline-secret".into(),
    ];
    let ctx = launch_context::from_capture(
        &c,
        ProcessIdentity {
            pid: 1,
            started_at: 2,
        },
        Source::ShellObserved,
    );
    let json = serde_json::to_string(&ctx).unwrap();
    assert!(!json.contains("fixture-secret-do-not-expose"));
    assert!(!json.contains("argv-secret"));
    assert!(!json.contains("inline-secret"));
    let repo = persistence::Persistence::open(&f.path.join("db.sqlite")).unwrap();
    repo.save_capture(&ctx, &c).unwrap();
    let loaded = repo.contexts().unwrap();
    assert!(!loaded[0].recoverable);
    assert!(loaded[0].reason.contains("not saved"));
    assert!(repo.execution(&ctx.id).unwrap().is_none());
    assert!(
        !launch_context::redact("token=another-secret fixture-secret-do-not-expose", &c)
            .contains("another-secret")
    );
}
#[test]
fn missing_cwd_executable_and_stale_identity_leave_owner_running() {
    let mut f = Fixture::new();
    let c = f.capture(&[port()]);
    let entry = f.start(&c);
    let engine = f.engine();
    let id = engine.observe(c, entry.identity().unwrap()).unwrap();
    let stale = ProcessIdentity {
        pid: entry.pid.unwrap(),
        started_at: entry.started_at.unwrap() - 1,
    };
    assert!(engine.restart(&id, stale, entry.port, false).is_err());
    assert!(resolver::alive(entry.identity().unwrap()));
    for missing_cwd in [true, false] {
        let mut store = engine.store.lock().unwrap();
        let record = store.records.get_mut(&id).unwrap();
        let mut capture = record.capture.clone().unwrap();
        if missing_cwd {
            capture.cwd = f.path.join("missing").to_string_lossy().into();
        } else {
            capture.executable = f.path.join("missing-bin").to_string_lossy().into();
        }
        record.capture = Some(capture);
        drop(store);
        assert!(engine
            .restart(&id, entry.identity().unwrap(), entry.port, false)
            .is_err());
        assert!(resolver::alive(entry.identity().unwrap()));
    }
}
#[cfg(unix)]
#[test]
fn restart_preserves_project_context_and_multiple_ports_and_detaches_lifecycle() {
    let mut f = Fixture::new();
    let ports = [port(), port()];
    let c = f.capture(&ports);
    let entry = f.start(&c);
    let engine = f.engine();
    let id = engine.observe(c, entry.identity().unwrap()).unwrap();
    let mut entries = engine.scan().unwrap();
    engine.enrich(&mut entries);
    let ours: Vec<_> = entries.iter().filter(|p| p.pid == entry.pid).collect();
    assert_eq!(ours.len(), 2);
    assert!(ours.iter().all(|p| p.launch.as_ref().unwrap().id == id));
    let project = ours[0].project.clone().unwrap();
    let result = engine.restart(&id, entry.identity().unwrap(), entry.port, false);
    if let Some(r) = engine.store.lock().unwrap().records.get(&id) {
        f.roots.push(r.context.launch_root);
    }
    let result = result.unwrap();
    assert_eq!(result.state, RecoveryState::Running);
    assert_ne!(result.new_pid, entry.pid);
    assert_eq!(result.ports.len(), 2);
    assert!(!result.output.contains("fixture-secret-do-not-expose"));
    let mut entries = engine.scan().unwrap();
    engine.enrich(&mut entries);
    for p in entries.iter().filter(|p| ports.contains(&p.port)) {
        assert_eq!(p.launch.as_ref().unwrap().id, id);
        assert_eq!(p.project.as_ref().unwrap().root_path, project.root_path);
    }
    drop(engine);
    assert!(
        resolver::alive(*f.roots.last().unwrap()),
        "Closing desktop state must not terminate the service"
    );
}
#[test]
fn pid_reuse_does_not_correlate() {
    let mut f = Fixture::new();
    let c = f.capture(&[port()]);
    let entry = f.start(&c);
    let mut identity = entry.identity().unwrap();
    identity.started_at -= 1;
    assert!(!correlation::contains(&entry, &[], identity));
}

#[cfg(unix)]
#[test]
fn stopped_launch_can_run_again_and_creates_grouped_history_without_duplicates() {
    let mut fixture = Fixture::new();
    let port = port();
    let capture = fixture.capture(&[port]);
    let entry = fixture.start(&capture);
    let engine = fixture.engine();
    let id = engine.observe(capture, entry.identity().unwrap()).unwrap();
    let mut entries = engine.scan().unwrap();
    engine.enrich(&mut entries);
    assert_eq!(
        engine
            .history(HistoryQuery {
                port: Some(port),
                ..Default::default()
            })
            .unwrap()
            .runs
            .len(),
        1
    );

    fixture.children.last_mut().unwrap().kill().unwrap();
    fixture.children.last_mut().unwrap().wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !port_free(port, &Scanner::new().scan().unwrap()) {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(30));
    }
    engine.enrich(&mut []);

    let result = engine.run_again(&id, true).unwrap();
    assert_eq!(result.state, RecoveryState::Running);
    let root = engine.store.lock().unwrap().records[&id]
        .context
        .launch_root;
    fixture.roots.push(root);

    let history = engine
        .history(HistoryQuery {
            port: Some(port),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(history.commands.len(), 1);
    let command = history
        .commands
        .iter()
        .find(|command| command.launch_context.id == id)
        .unwrap();
    assert_eq!(command.run_count, 2);
    assert_eq!(history.commands[0].typical_ports, vec![port]);
    assert!(history.commands[0].active);
    assert!(engine
        .run_again(&id, true)
        .unwrap_err()
        .contains("already be running"));

    engine.pin_command(port, &id, true).unwrap();
    let history = engine
        .history(HistoryQuery {
            port: Some(port),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(history.commands[0].pinned_ports, vec![port]);
    for run in history.runs {
        engine.remove_run(&run.id).unwrap();
    }
    let history = engine
        .history(HistoryQuery {
            port: Some(port),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(history.commands.len(), 1);
    assert_eq!(history.commands[0].run_count, 0);
    assert_eq!(history.commands[0].pinned_ports, vec![port]);
}

#[cfg(unix)]
#[test]
fn observed_npm_listener_restarts_the_npm_launch_root_not_the_child() {
    let Some(npm) = crate::autopilot::capture::resolve_executable("npm", Path::new("/")) else {
        return;
    };
    let mut f = Fixture::new();
    let port = port();
    let mut c = f.capture(&[port]);
    let script = format!(
        "exec {} --ignored --exact recovery::tests::listener_fixture --nocapture",
        launch_context::display(&[c.executable.clone()])
    );
    let vite = Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/vite/bin/vite.js");
    let script = if vite.is_file() {
        format!(
            "node {} --host 127.0.0.1 --port {port} --strictPort",
            launch_context::display(&[vite.to_string_lossy().into_owned()])
        )
    } else {
        script
    };
    fs::write(
        f.path.join("package.json"),
        serde_json::to_vec(&serde_json::json!({"name":"shipwreck","scripts":{"dev":script}}))
            .unwrap(),
    )
    .unwrap();
    c.executable = npm.to_string_lossy().into();
    c.argv = vec!["npm".into(), "run".into(), "dev".into()];
    let child = c
        .command()
        .unwrap()
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    f.children.push(child);
    let deadline = Instant::now() + Duration::from_secs(8);
    let listener = loop {
        if let Some(p) = Scanner::new()
            .scan()
            .unwrap()
            .into_iter()
            .find(|p| p.port == port)
        {
            break p;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(50));
    };
    let root = ancestry::ancestors(&listener)
        .into_iter()
        .find(|p| p.identity.pid == pid)
        .unwrap()
        .identity;
    assert_ne!(root.pid, listener.pid.unwrap());
    let engine = f.engine();
    let id = engine.observe(c, root).unwrap();
    let mut entries = engine.scan().unwrap();
    engine.enrich(&mut entries);
    let context = entries
        .iter()
        .find(|p| p.port == port)
        .unwrap()
        .launch
        .as_ref()
        .unwrap();
    assert_eq!(context.launch_root, root);
    assert_eq!(context.command, "npm run dev");
    engine
        .save_profile(LaunchProfile {
            project_id: f.path.to_string_lossy().into(),
            name: "Fallback".into(),
            executable: "/bin/sleep".into(),
            args: vec!["30".into()],
            working_directory: f.path.to_string_lossy().into(),
        })
        .unwrap();
    engine.enrich(&mut entries);
    assert_eq!(
        entries
            .iter()
            .find(|p| p.port == port)
            .unwrap()
            .launch
            .as_ref()
            .unwrap()
            .command,
        "npm run dev",
        "A recipe must not replace a fully observed launch"
    );
    let result = engine.restart(&id, listener.identity().unwrap(), port, false);
    f.roots.push(
        engine.store.lock().unwrap().records[&id]
            .context
            .launch_root,
    );
    let result = result.unwrap();
    assert_eq!(result.state, RecoveryState::Running);
    assert_ne!(result.new_pid, listener.pid);
    assert!(!resolver::alive(root));
}

#[test]
#[ignore = "profile listener fixture"]
fn profile_listener_fixture() {
    let port: u16 = fs::read_to_string("port.txt").unwrap().parse().unwrap();
    let _listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
#[cfg(unix)]
#[test]
fn project_recipe_test_refuses_duplicates_then_launches_and_verifies() {
    let mut f = Fixture::new();
    let port = port();
    let c = f.capture(&[port]);
    let entry = f.start(&c);
    let engine = f.engine();
    let entries = engine.scan().unwrap();
    assert!(entries
        .iter()
        .find(|p| p.pid == entry.pid)
        .unwrap()
        .project
        .is_some());
    let project = f.path.to_string_lossy().into_owned();
    let profile = LaunchProfile {
        project_id: project.clone(),
        name: "Development".into(),
        executable: std::env::current_exe().unwrap().to_string_lossy().into(),
        args: vec![
            "--ignored".into(),
            "--exact".into(),
            "recovery::tests::profile_listener_fixture".into(),
            "--nocapture".into(),
        ],
        working_directory: project.clone(),
    };
    engine.save_profile(profile).unwrap();
    assert!(engine
        .test_profile(&project, port)
        .unwrap_err()
        .contains("already running"));
    assert!(resolver::alive(entry.identity().unwrap()));
    f.children[0].kill().unwrap();
    f.children[0].wait().unwrap();
    fs::write(f.path.join("port.txt"), port.to_string()).unwrap();
    let (id, result) = engine.test_profile(&project, port).unwrap();
    f.roots.push(
        engine.store.lock().unwrap().records[&id]
            .context
            .launch_root,
    );
    assert_eq!(result.state, RecoveryState::Running);
    let durable = f.engine();
    assert_eq!(
        durable.profile(&project).unwrap().args[2],
        "recovery::tests::profile_listener_fixture"
    );
}

#[cfg(unix)]
#[test]
fn executable_resolution_preserves_virtualenv_style_symlinks() {
    let f = Fixture::new();
    let interpreter = f.path.join("python");
    std::os::unix::fs::symlink("/bin/sh", &interpreter).unwrap();
    let resolved = crate::autopilot::capture::resolve_executable("./python", &f.path).unwrap();
    assert_eq!(resolved, f.path.join("./python"));
    assert_ne!(resolved, interpreter.canonicalize().unwrap());
}

#[test]
fn quoted_and_multiline_secrets_are_redacted() {
    let f = Fixture::new();
    let mut capture = f.capture(&[]);
    capture.env.clear();
    capture.env.insert(
        "PRIVATE_KEY".into(),
        "-----BEGIN PRIVATE KEY-----\nsuper-private-key-line\n-----END PRIVATE KEY-----".into(),
    );
    for text in [
        "super-private-key-line\n",
        r#"PASSWORD="a secret with spaces""#,
        r#"{"API_TOKEN": "quoted-json-value"}"#,
    ] {
        let redacted = launch_context::redact(text, &capture);
        assert!(!redacted.contains("super-private-key-line"));
        assert!(!redacted.contains("secret with spaces"));
        assert!(!redacted.contains("quoted-json-value"));
    }
}

#[test]
fn parent_metadata_does_not_replay_package_manager_arguments_as_node_arguments() {
    assert!(!adapters::compatible(
        &["npm".into(), "run".into(), "dev".into()],
        Path::new("/usr/bin/node")
    ));
    assert!(adapters::compatible(
        &[
            "node".into(),
            "/tools/npm-cli.js".into(),
            "run".into(),
            "dev".into()
        ],
        Path::new("/usr/bin/node")
    ));
    assert!(adapters::compatible(
        &["python3".into(), "-m".into(), "uvicorn".into()],
        Path::new("/usr/bin/python3.12")
    ));
}

#[test]
fn display_redacts_values_by_context_not_environment_substrings() {
    let f = Fixture::new();
    let mut c = f.capture(&[]);
    for (key, value) in [
        ("SMALL", "6"),
        ("DIRECTORY", "bin"),
        ("MODE", "dev"),
        ("TOOL", "node"),
        ("OTHER", "usr"),
        ("AUTH_TOKEN", "6"),
    ] {
        c.env.insert(key.into(), value.into());
    }
    for args in [
        vec![
            "node",
            "/Users/user/Code/port-authority/node_modules/.bin/vite",
            "--host",
            "localhost",
            "--port",
            "5173",
        ],
        vec!["npm", "run", "dev"],
        vec!["pnpm", "dev"],
        vec!["cargo", "tauri", "dev"],
        vec![
            "next-server (v16.6.6)",
            "NVM_INC=/Users/user/.nvm/versions/node/v22/include/node",
        ],
        vec!["/Users/user/Code/rippley-labs", "--watch", "--inspect"],
    ] {
        c.argv = args.iter().map(|s| s.to_string()).collect();
        assert_eq!(launch_context::safe_argv(&c), c.argv);
        assert_eq!(launch_context::redact(&args.join(" "), &c), args.join(" "));
    }
    c.argv = [
        "tool",
        "GITHUB_TOKEN=ghp_abcdef",
        "API_KEY=123456",
        "--token",
        "secret-value",
        "--password=hunter2",
        "--host",
        "localhost",
    ]
    .map(String::from)
    .into();
    let (safe, decisions) = sanitizer::arguments(&c);
    for secret in ["ghp_abcdef", "123456", "secret-value", "hunter2"] {
        assert!(!safe.join(" ").contains(secret));
    }
    assert_eq!(&safe[6..], &["--host", "localhost"]);
    assert_eq!(decisions[4].classification, "sensitive_option_value");
    assert_eq!(decisions[7].classification, "safe");
    assert!(!serde_json::to_string(&decisions)
        .unwrap()
        .contains("secret-value"));
    c.argv = vec![
        "sh".into(),
        "-c".into(),
        "curl --token 'secret-value' --url=https://user:password@example.test/?token=value".into(),
    ];
    let text = launch_context::safe_argv(&c).join(" ");
    assert!(!text.contains("secret-value"));
    assert!(!text.contains("user:password"));
    assert!(!text.contains("token=value"));
}

#[test]
fn durable_execution_preserves_managers_arguments_cwd_and_shell() {
    let f = Fixture::new();
    let repo = persistence::Persistence::open(&f.path.join("durable.sqlite")).unwrap();
    let mut c = f.capture(&[]);
    for args in [
        vec!["npm", "run", "dev"],
        vec!["pnpm", "dev"],
        vec!["cargo", "tauri", "dev"],
        vec!["/bin/sh", "-c", "FOO=bar printf '%s' 'two words'"],
    ] {
        let executable = f
            .path
            .join(std::path::Path::new(args[0]).file_name().unwrap());
        fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        c.executable = executable.to_string_lossy().into();
        c.argv = args.iter().map(|s| s.to_string()).collect();
        let display = launch_context::from_capture(
            &c,
            ProcessIdentity {
                pid: 999999,
                started_at: 1,
            },
            Source::ShellObserved,
        );
        repo.save_capture(&display, &c).unwrap();
        let saved = repo.execution(&display.id).unwrap().unwrap();
        let payload = serde_json::to_string(&saved).unwrap();
        assert!(!payload.contains("fixture-secret-do-not-expose"));
        assert!(!payload.contains("API_TOKEN"));
        let restored = saved.capture().unwrap();
        assert_eq!(restored.argv, c.argv);
        assert_eq!(restored.cwd, c.cwd);
        assert_eq!(restored.executable, c.executable);
        let command = restored.command().unwrap();
        assert_eq!(command.get_program(), std::ffi::OsStr::new(&c.executable));
        assert_eq!(
            command
                .get_args()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>(),
            &args[1..]
        );
        assert_eq!(command.get_current_dir(), Some(f.path.as_path()));
    }
    let mut masked = c.clone();
    masked.argv.push(sanitizer::MASK.into());
    assert!(masked.command().unwrap_err().contains("Masked display"));
}

#[test]
fn legacy_metadata_is_never_an_execution_source_and_moved_cwd_is_rejected() {
    let f = Fixture::new();
    let repo = persistence::Persistence::open(&f.path.join("legacy.sqlite")).unwrap();
    let c = f.capture(&[]);
    let mut display = launch_context::from_capture(
        &c,
        ProcessIdentity {
            pid: 999999,
            started_at: 1,
        },
        Source::ShellObserved,
    );
    display.schema_version = 0;
    display.command = "node ••••••••".into();
    repo.save(&display).unwrap();
    let legacy = repo.contexts().unwrap().remove(0);
    assert!(!legacy.recoverable);
    assert_eq!(legacy.recovery_confidence, RecoveryConfidence::Unavailable);
    assert!(legacy.reason.contains("Legacy history entry"));
    assert!(repo.execution(&legacy.id).unwrap().is_none());
    let legacy_engine = Recovery::new(
        &f.path.join("legacy.sqlite"),
        f.engine().scanner.clone(),
        f.engine().projects.clone(),
    );
    let resolved = legacy_engine.context(&legacy.id).unwrap();
    assert!(!resolved.recoverable);
    assert!(resolved.reason.contains("Legacy history entry"));
    assert!(legacy_engine.context("missing-timeline-context").is_err());
    let original = f.path.join("original");
    fs::create_dir(&original).unwrap();
    let mut c = c;
    c.cwd = original.to_string_lossy().into();
    let saved = execution::LaunchContext::from_capture(&display, &c).unwrap();
    fs::rename(&original, f.path.join("moved")).unwrap();
    fs::create_dir(&original).unwrap();
    assert!(saved
        .validate_directory()
        .unwrap_err()
        .contains("moved or replaced"));
}

#[cfg(unix)]
#[test]
fn stopped_launch_survives_app_restart_and_ignores_display_text() {
    let mut f = Fixture::new();
    let port = port();
    fs::write(f.path.join("port.txt"), port.to_string()).unwrap();
    let mut c = f.capture(&[]);
    c.argv[3] = "recovery::tests::profile_listener_fixture".into();
    let entry = f.start(&c);
    let engine = f.engine();
    let id = engine
        .observe(c.clone(), entry.identity().unwrap())
        .unwrap();
    let mut entries = engine.scan().unwrap();
    engine.enrich(&mut entries);
    let first_run = entries
        .iter()
        .find(|e| e.port == port)
        .unwrap()
        .launch
        .as_ref()
        .unwrap()
        .run_id
        .clone()
        .unwrap();
    // A child listener must never replace the launch root or make repeated scans new runs.
    engine.enrich(&mut entries);
    assert_eq!(
        engine.history(HistoryQuery::default()).unwrap().runs.len(),
        1
    );
    engine.pin_command(port, &id, true).unwrap();
    {
        let persistence = engine.persistence.lock().unwrap();
        let repo = persistence.as_ref().unwrap();
        let before = repo
            .runs(100)
            .unwrap()
            .into_iter()
            .find(|run| run.launch_context_id == id)
            .unwrap();
        assert!(before.project_name.is_some());
        let mut worker = entries
            .iter()
            .find(|entry| entry.port == port)
            .unwrap()
            .clone();
        worker.project = None;
        repo.observe_port(worker.launch.as_ref().unwrap(), &worker)
            .unwrap();
        let after = repo
            .runs(100)
            .unwrap()
            .into_iter()
            .find(|run| run.launch_context_id == id)
            .unwrap();
        assert_eq!(
            after.project_name, before.project_name,
            "A worker with missing metadata must not erase the project name"
        );
    }
    f.children[0].kill().unwrap();
    f.children[0].wait().unwrap();
    drop(engine);
    let engine = f.engine();
    let history = engine.history(HistoryQuery::default()).unwrap();
    let command = history
        .commands
        .iter()
        .find(|command| command.launch_context.id == id)
        .unwrap();
    assert!(!command.active);
    assert!(command.launch_context.recoverable);
    // Timeline snapshots resolve current availability by ID after an app restart.
    let resolved = engine.context(&id).unwrap();
    assert!(resolved.recoverable);
    assert_eq!(resolved.args, c.argv[1..]);
    assert_eq!(command.pinned_ports, vec![port]);
    assert_eq!(command.latest_run.state, CommandRunState::Stopped);
    {
        let mut store = engine.store.lock().unwrap();
        let record = store.records.get_mut(&id).unwrap();
        assert_eq!(record.capture.as_ref().unwrap().argv, c.argv);
        record.context.command = "DO NOT EXECUTE ••••••••".into();
        record.context.args = vec![sanitizer::MASK.into()];
    }
    let result = engine.run_again(&id, true);
    f.roots.push(
        engine.store.lock().unwrap().records[&id]
            .context
            .launch_root,
    );
    assert_eq!(result.unwrap().state, RecoveryState::Running);
    let history = engine.history(HistoryQuery::default()).unwrap();
    let command = history
        .commands
        .iter()
        .find(|command| command.launch_context.id == id)
        .unwrap();
    assert_eq!(command.run_count, 2);
    assert_ne!(command.latest_run.id, first_run);
    assert_ne!(command.latest_run.process_identity, entry.identity());
    assert!(engine
        .run_again(&id, true)
        .unwrap_err()
        .contains("already be running"));
    // Eviction from the RAM cache cannot delete the durable command.
    engine.store.lock().unwrap().records.remove(&id);
    engine.restore_record(&id).unwrap();
    assert_eq!(
        engine.store.lock().unwrap().records[&id]
            .capture
            .as_ref()
            .unwrap()
            .argv,
        c.argv
    );
}

/// Opt-in dogfood check against a real project, with an isolated history database.
/// PA_DOGFOOD_SPEC is a JSON file containing cwd, argv, and expected port.
#[cfg(unix)]
#[test]
#[ignore = "requires an explicitly selected local development project"]
fn real_project_history_dogfood() {
    let spec: serde_json::Value = serde_json::from_slice(
        &fs::read(std::env::var("PA_DOGFOOD_SPEC").expect("Set PA_DOGFOOD_SPEC")).unwrap(),
    )
    .unwrap();
    let mut f = Fixture::new();
    let expected: u16 = spec["port"].as_u64().unwrap().try_into().unwrap();
    assert!(
        port_free(expected, &Scanner::new().scan().unwrap()),
        "Dogfood port is already occupied; leave its owner untouched"
    );
    let mut capture = f.capture(&[]);
    capture.cwd = Path::new(spec["cwd"].as_str().unwrap())
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into();
    capture.argv = spec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect();
    capture.executable =
        crate::autopilot::capture::resolve_executable(&capture.argv[0], Path::new(&capture.cwd))
            .unwrap()
            .to_string_lossy()
            .into();
    capture.env = std::env::vars().collect();
    let initial_output = fs::File::create(f.path.join("initial.log")).unwrap();
    let child = capture
        .command()
        .unwrap()
        .stdout(initial_output.try_clone().unwrap())
        .stderr(initial_output)
        .spawn()
        .unwrap();
    let pid = child.id();
    f.children.push(child);
    thread::sleep(Duration::from_millis(200));
    let mut sys = sysinfo::System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
        true,
        crate::process::inspector::refresh_kind(),
    );
    let root = ProcessIdentity {
        pid,
        started_at: sys
            .process(sysinfo::Pid::from_u32(pid))
            .unwrap()
            .start_time(),
    };
    f.roots.push(root);
    let engine = Recovery::new(
        &spec["database"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| f.path.join("timeline.sqlite")),
        Arc::new(Mutex::new(Scanner::new())),
        ProjectEngine::new(None),
    );
    let id = engine.observe(capture.clone(), root).unwrap();
    let deadline = Instant::now() + Duration::from_secs(150);
    let ready_process = |root| {
        spec["expectedProcess"].as_str().is_none_or(|name| {
            ancestry::inspect_tree(root)
                .is_ok_and(|tree| tree.iter().any(|member| member.name == name))
        })
    };
    let first_run = loop {
        let mut entries = engine.scan().unwrap();
        engine.enrich(&mut entries);
        if let Some(entry) = entries.iter().find(|entry| {
            entry.port == expected && entry.launch.as_ref().is_some_and(|launch| launch.id == id)
        }) {
            if !ready_process(root) {
                assert!(
                    Instant::now() < deadline,
                    "Development desktop did not start"
                );
                thread::sleep(Duration::from_millis(300));
                continue;
            }
            assert_eq!(entry.project.as_ref().unwrap().root_path, capture.cwd);
            assert_eq!(
                entry.launch.as_ref().unwrap().command,
                launch_context::display(&capture.argv)
            );
            break entry.launch.as_ref().unwrap().run_id.clone().unwrap();
        }
        assert!(
            resolver::alive(root),
            "Development command exited before its port appeared"
        );
        assert!(Instant::now() < deadline, "Development port did not appear");
        thread::sleep(Duration::from_millis(300));
    };
    // Signal only this test's captured tree, with process-start identity validation.
    safety::stop(&ancestry::inspect_tree(root).unwrap(), false).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while resolver::alive(root) || !port_free(expected, &Scanner::new().scan().unwrap()) {
        assert!(
            Instant::now() < deadline,
            "Test-owned development command did not stop"
        );
        thread::sleep(Duration::from_millis(100));
    }
    drop(engine);
    let engine = Recovery::new(
        &spec["database"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| f.path.join("timeline.sqlite")),
        Arc::new(Mutex::new(Scanner::new())),
        ProjectEngine::new(None),
    );
    let history = engine.history(HistoryQuery::default()).unwrap();
    let previous = history
        .commands
        .iter()
        .find(|command| command.launch_context.id == id)
        .unwrap();
    assert!(!previous.active);
    assert!(previous.launch_context.recoverable);
    let result = engine.run_again(&id, true);
    let new_root = engine.store.lock().unwrap().records[&id]
        .context
        .launch_root;
    f.roots.push(new_root);
    let result = result.unwrap();
    let deadline = Instant::now() + Duration::from_secs(150);
    while !ready_process(new_root) {
        assert!(
            resolver::alive(new_root) && Instant::now() < deadline,
            "Relaunched desktop did not start"
        );
        thread::sleep(Duration::from_millis(300));
    }
    assert_eq!(result.state, RecoveryState::Running);
    assert_eq!(result.ports, vec![expected]);
    let history = engine.history(HistoryQuery::default()).unwrap();
    let command = history
        .commands
        .iter()
        .find(|command| command.launch_context.id == id)
        .unwrap();
    assert_eq!(command.run_count, 2);
    assert_eq!(
        command.launch_context.command,
        launch_context::display(&capture.argv)
    );
    assert_ne!(command.latest_run.id, first_run);
    assert_eq!(
        command.latest_run.project_id.as_deref(),
        Some(capture.cwd.as_str())
    );
    assert_eq!(
        engine.store.lock().unwrap().records[&id]
            .capture
            .as_ref()
            .unwrap()
            .argv,
        capture.argv
    );
    println!(
        "Verified {} in {}: stopped, reopened history, relaunched PID {}, observed :{}, new run {}",
        command.launch_context.command, capture.cwd, new_root.pid, expected, command.latest_run.id
    );
}

#[test]
fn runs_started_in_the_same_second_keep_creation_order_after_updates() {
    let f = Fixture::new();
    let repo = persistence::Persistence::open(&f.path.join("ordering.sqlite")).unwrap();
    let c = f.capture(&[]);
    let mut context = launch_context::from_capture(
        &c,
        ProcessIdentity {
            pid: 100,
            started_at: 42,
        },
        Source::ShellObserved,
    );
    let first = repo.start_run(&context).unwrap();
    context.launch_root.pid = 101;
    let second = repo.start_run(&context).unwrap();
    let mut first = first;
    first.state = CommandRunState::Stopped;
    repo.save_run(&first).unwrap();
    assert_eq!(repo.runs(10).unwrap()[0].id, second.id);
}

#[test]
fn next_service_verification_uses_configured_port_not_random_worker_sockets() {
    let f = Fixture::new();
    fs::write(
        f.path.join("package.json"),
        r#"{"scripts":{"dev":"next dev --hostname 127.0.0.1 --port 3077"}}"#,
    )
    .unwrap();
    let mut c = f.capture(&[]);
    c.env.remove("PORT");
    for tool in ["npm", "pnpm"] {
        c.argv = vec![tool.into(), "run".into(), "dev".into()];
        assert_eq!(launch_context::configured_ports(&c), vec![3077]);
    }
    c.argv
        .extend(["--".into(), "--port".into(), "60123".into()]);
    assert_eq!(
        launch_context::configured_ports(&c),
        vec![60123],
        "explicit high service ports must remain valid"
    );
    c.argv.truncate(3);
    fs::write(
        f.path.join("package.json"),
        r#"{"scripts":{"dev":"echo hi && next dev --port 3077"}}"#,
    )
    .unwrap();
    assert!(
        launch_context::configured_ports(&c).is_empty(),
        "do not interpret arbitrary shell configuration"
    );
}

#[test]
fn stale_app_instance_cannot_stop_a_newer_run() {
    let f = Fixture::new();
    let repo = persistence::Persistence::open(&f.path.join("concurrency.sqlite")).unwrap();
    let c = f.capture(&[]);
    let old_root = ProcessIdentity {
        pid: 100,
        started_at: 42,
    };
    let mut context = launch_context::from_capture(&c, old_root, Source::ShellObserved);
    repo.start_run(&context).unwrap();
    context.launch_root = ProcessIdentity {
        pid: 101,
        started_at: 43,
    };
    let new_run = repo.start_run(&context).unwrap();
    repo.finish_observed_root(&context.id, old_root, "Old process exited")
        .unwrap();
    let runs = repo.runs(10).unwrap();
    assert_eq!(runs[0].id, new_run.id);
    assert_eq!(runs[0].state, CommandRunState::Running);
    assert_eq!(repo.start_run(&context).unwrap().id, new_run.id);
}
