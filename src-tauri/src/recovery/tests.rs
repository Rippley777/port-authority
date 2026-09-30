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
    repo.save(&ctx).unwrap();
    let loaded = repo.contexts().unwrap();
    assert!(!loaded[0].recoverable);
    assert!(loaded[0].reason.contains("did not store"));
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
