use super::*;
use crate::autopilot::{classifier, models::Category};
use crate::ports::scanner::Scanner;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Stdio},
};
const FIXTURE: &str = "autopilot::engine::tests::fixture_server";
#[test]
#[ignore = "subprocess fixture, invoked only by the integration tests"]
fn fixture_server() {
    if let Ok(expected) = std::env::var("PA_EXPECT_CWD") {
        assert_eq!(std::env::current_dir().unwrap(), PathBuf::from(expected));
    }
    if std::env::var("PA_CHECK_ENV").is_ok() {
        assert_eq!(
            std::env::var("PA_SECRET_TEST").unwrap(),
            "preserved-not-for-the-ui"
        );
    }
    if std::env::var("PA_FIXTURE_IGNORE_TERM").is_ok() {
        // SAFETY: isolated test subprocess installs a standard SIG_IGN disposition.
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }
    let port: u16 = std::env::var("PA_FIXTURE_PORT").unwrap().parse().unwrap();
    let _listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(_) => {
            eprintln!("Error: listen EADDRINUSE: address already in use 127.0.0.1:{port}");
            std::process::exit(23);
        }
    };
    println!("CONTEXT_OK: listening on {port}");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
struct Workspace {
    path: PathBuf,
}
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pa-autopilot-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
    fn capture(&self, port: u16) -> Capture {
        let executable = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut env: BTreeMap<_, _> = std::env::vars().collect();
        env.insert("PA_FIXTURE_PORT".into(), port.to_string());
        env.insert(
            "PA_EXPECT_CWD".into(),
            self.path
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        env.insert("PA_CHECK_ENV".into(), "1".into());
        env.insert("PA_SECRET_TEST".into(), "preserved-not-for-the-ui".into());
        Capture {
            argv: vec![
                executable.clone(),
                "--ignored".into(),
                "--exact".into(),
                FIXTURE.into(),
                "--nocapture".into(),
            ],
            executable,
            cwd: self.path.to_string_lossy().into_owned(),
            env,
            error: format!("Error: listen EADDRINUSE ::: {port}").replace("::: ", ":::"),
            exit_code: 23,
        }
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
struct Owner(Child);
impl Owner {
    fn new(capture: &Capture, ignore: bool) -> Self {
        let mut command = capture.command().unwrap();
        command.stdout(Stdio::null()).stderr(Stdio::null());
        if ignore {
            command.env("PA_FIXTURE_IGNORE_TERM", "1");
        }
        let child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let port: u16 = capture.env["PA_FIXTURE_PORT"].parse().unwrap();
        loop {
            if Scanner::new()
                .scan()
                .unwrap()
                .iter()
                .any(|p| p.port == port && p.pid == Some(child.id()))
            {
                break;
            }
            assert!(Instant::now() < deadline, "Fixture did not bind");
            thread::sleep(Duration::from_millis(20));
        }
        Self(child)
    }
    fn stop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Engine(Autopilot);
impl Engine {
    fn new() -> Self {
        let engine = Autopilot::new(Arc::new(Mutex::new(Scanner::new())), String::new());
        engine.enable(true).unwrap();
        Self(engine)
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if let Ok(store) = self.0.store.lock() {
            for run in &store.launches {
                run.stop_for_test();
            }
        }
    }
}
fn free_port() -> u16 {
    // Stay below the OS ephemeral range: parallel socket probes must not make
    // the kernel allocate a test's suggested alternate port to another fixture.
    for _ in 0..200 {
        let port = 10000 + (Uuid::new_v4().as_u128() % 20000) as u16;
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
            return listener.local_addr().unwrap().port();
        }
    }
    panic!("No isolated fixture port available");
}
fn view(engine: &Autopilot, id: &str) -> Conflict {
    engine
        .snapshot()
        .unwrap()
        .conflicts
        .into_iter()
        .find(|c| c.id == id)
        .unwrap()
}

#[test]
fn graceful_recovery_preserves_context_verifies_owner_and_never_serializes_environment() {
    let workspace = Workspace::new();
    let port = free_port();
    let capture = workspace.capture(port);
    let mut owner = Owner::new(&capture, false);
    let engine = Engine::new();
    let id = engine.0.register(capture).unwrap();
    assert!(engine.0.act(&id, Action::KillRetry, false, None).is_err());
    assert!(
        owner.0.try_wait().unwrap().is_none(),
        "Unapproved owner must remain alive"
    );
    engine.0.act(&id, Action::KillRetry, true, None).unwrap();
    let conflict = view(&engine.0, &id);
    assert_eq!(conflict.status, Status::Resolved);
    assert_ne!(conflict.launched_pid, Some(owner.0.id()));
    assert!(conflict.output.contains("CONTEXT_OK"));
    assert!(conflict
        .steps
        .iter()
        .any(|s| s.contains("Verified that the new process")));
    let json = serde_json::to_string(&engine.0.snapshot().unwrap()).unwrap();
    assert!(!json.contains("preserved-not-for-the-ui"));
    assert!(!json.contains("PA_SECRET_TEST"));
    assert!(
        engine.0.act(&id, Action::Retry, true, None).is_err(),
        "Cannot spawn duplicate while retry is running"
    );
}
#[test]
fn changed_owner_is_not_killed_and_forced_recovery_is_not_available_early() {
    let workspace = Workspace::new();
    let port = free_port();
    let capture = workspace.capture(port);
    let mut original = Owner::new(&capture, false);
    let engine = Engine::new();
    let id = engine.0.register(capture.clone()).unwrap();
    assert!(engine.0.act(&id, Action::ForceRetry, true, None).is_err());
    original.stop();
    let mut replacement = Owner::new(&capture, false);
    assert!(engine
        .0
        .act(&id, Action::KillRetry, true, None)
        .unwrap_err()
        .contains("changed"));
    assert!(replacement.0.try_wait().unwrap().is_none());
}
#[test]
fn ignored_graceful_signal_requires_separate_force_approval() {
    let workspace = Workspace::new();
    let port = free_port();
    let capture = workspace.capture(port);
    let mut owner = Owner::new(&capture, true);
    let engine = Engine::new();
    let id = engine.0.register(capture).unwrap();
    engine.0.act(&id, Action::KillRetry, true, None).unwrap();
    assert_eq!(view(&engine.0, &id).status, Status::ForceRequired);
    assert!(engine.0.act(&id, Action::ForceRetry, false, None).is_err());
    assert!(owner.0.try_wait().unwrap().is_none());
    engine.0.act(&id, Action::ForceRetry, true, None).unwrap();
    assert_eq!(view(&engine.0, &id).status, Status::Resolved);
}
#[test]
fn alternate_port_preserves_owner_and_rewrites_only_supported_arguments() {
    let workspace = Workspace::new();
    let port = free_port();
    let mut capture = workspace.capture(port);
    let mut owner = Owner::new(&capture, false);
    let script = workspace.path.join("vite");
    fs::write(&script,"#!/bin/sh\nif [ \"$1\" = --port ]; then export PA_FIXTURE_PORT=\"$2\"; fi\nexec \"$PA_TEST_EXE\" --ignored --exact autopilot::engine::tests::fixture_server --nocapture\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    capture
        .env
        .insert("PA_TEST_EXE".into(), capture.executable.clone());
    capture.executable = script.to_string_lossy().into_owned();
    capture.argv = vec!["vite".into(), "--port".into(), port.to_string()];
    let engine = Engine::new();
    let id = engine.0.register(capture).unwrap();
    let alternate = view(&engine.0, &id).alternative.unwrap();
    engine
        .0
        .act(&id, Action::Alternate, false, Some(alternate))
        .unwrap();
    assert_eq!(view(&engine.0, &id).status, Status::Resolved);
    assert!(owner.0.try_wait().unwrap().is_none());
    assert!(Scanner::new()
        .scan()
        .unwrap()
        .iter()
        .any(|p| p.port == alternate));
}
#[test]
fn restart_owner_uses_only_an_actual_managed_capture() {
    let workspace = Workspace::new();
    let port = free_port();
    let capture = workspace.capture(port);
    let _owner = Owner::new(&capture, false);
    let engine = Engine::new();
    let first = engine.0.register(capture.clone()).unwrap();
    assert!(!view(&engine.0, &first).restart_owner_available);
    assert!(engine
        .0
        .act(&first, Action::RestartOwner, true, None)
        .is_err());
    engine.0.act(&first, Action::KillRetry, true, None).unwrap();
    let first_pid = view(&engine.0, &first).launched_pid;
    let second = engine.0.register(capture).unwrap();
    assert!(view(&engine.0, &second).restart_owner_available);
    engine
        .0
        .act(&second, Action::RestartOwner, true, None)
        .unwrap();
    let restarted = view(&engine.0, &second);
    assert_eq!(restarted.status, Status::Resolved);
    assert_ne!(restarted.launched_pid, first_pid);
    assert!(restarted
        .message
        .contains("blocked project has not been retried"));
}
#[test]
fn retry_failure_reports_actual_exit_and_output() {
    let workspace = Workspace::new();
    let port = free_port();
    let mut capture = workspace.capture(port);
    let _owner = Owner::new(&capture, false);
    capture.executable = "/bin/sh".into();
    capture.argv = vec![
        "sh".into(),
        "-c".into(),
        "echo actual-retry-error >&2; exit 42".into(),
    ];
    let engine = Engine::new();
    let id = engine.0.register(capture).unwrap();
    let error = engine
        .0
        .act(&id, Action::KillRetry, true, None)
        .unwrap_err();
    assert!(error.contains("42"));
    assert!(error.contains("actual-retry-error"));
    assert_eq!(view(&engine.0, &id).status, Status::Failed);
}
#[test]
fn classifier_distinguishes_dev_infrastructure_and_blocked_identity() {
    let workspace = Workspace::new();
    let port = free_port();
    let capture = workspace.capture(port);
    let owner = Owner::new(&capture, false);
    let mut entry = Scanner::new()
        .scan()
        .unwrap()
        .into_iter()
        .find(|p| p.pid == Some(owner.0.id()))
        .unwrap();
    entry.process = "node".into();
    entry.command = vec![
        "node".into(),
        "/project/node_modules/vite/bin/vite.js".into(),
    ];
    let dev = classifier::classify(Some(&entry), &capture.cwd, true);
    assert_eq!(dev.category, Category::DevServer);
    assert_eq!(dev.risk, Risk::Low);
    entry.process = "postgres".into();
    entry.command = vec!["postgres".into()];
    let infra = classifier::classify(Some(&entry), &capture.cwd, true);
    assert_eq!(infra.category, Category::Infrastructure);
    assert_eq!(infra.risk, Risk::High);
    entry.protected = true;
    assert_eq!(
        classifier::classify(Some(&entry), &capture.cwd, true).risk,
        Risk::Blocked
    );
    entry.protected = false;
    entry.executable = None;
    assert_eq!(
        classifier::classify(Some(&entry), &capture.cwd, true).category,
        Category::Unknown
    );
}
#[test]
fn shell_capture_preserves_failure_code_and_exports_without_evaluation() {
    let workspace = Workspace::new();
    // A literal external command; shell operators in arguments are not interpreted by the wrapper.
    let script = workspace.path.join("fails");
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' 'Error: listen EADDRINUSE :::5173' >&2\nexit 23\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let code = crate::autopilot::shell::run_with_submit(
        vec![
            script.to_string_lossy().into_owned(),
            "literal argument ; $(not-a-command)".into(),
        ],
        |capture| {
            assert_eq!(capture.argv[1], "literal argument ; $(not-a-command)");
            assert_eq!(capture.exit_code, 23);
            assert!(capture.env.contains_key("PATH"));
            assert_eq!(
                capture.cwd,
                std::env::current_dir().unwrap().to_string_lossy()
            );
            Ok("test".into())
        },
    );
    assert_eq!(code, 23);
}
#[test]
fn captures_expire_ignore_clears_context_and_unknown_adapters_are_rejected() {
    let workspace = Workspace::new();
    let capture = workspace.capture(free_port());
    let engine = Engine::new();
    let id = engine.0.register(capture.clone()).unwrap();
    assert!(capture.alternate(5174).is_none());
    engine
        .0
        .store
        .lock()
        .unwrap()
        .records
        .get_mut(&id)
        .unwrap()
        .view
        .expires_at = now() - 1;
    assert_eq!(view(&engine.0, &id).status, Status::Expired);
    assert!(engine.0.act(&id, Action::Retry, true, None).is_err());
    engine.0.act(&id, Action::Ignore, false, None).unwrap();
    assert!(engine.0.snapshot().unwrap().conflicts.is_empty());
    engine.0.enable(false).unwrap();
    assert!(engine.0.register(capture).is_err());
}
#[test]
fn adapter_handles_npm_delimiters_and_rejects_compound_scripts() {
    let workspace = Workspace::new();
    let mut capture = workspace.capture(5173);
    capture.argv = vec!["npm".into(), "run".into(), "dev".into()];
    fs::write(
        workspace.path.join("package.json"),
        r#"{"scripts":{"dev":"vite"}}"#,
    )
    .unwrap();
    assert_eq!(
        capture.alternate(5174).unwrap().argv,
        vec!["npm", "run", "dev", "--", "--port", "5174"]
    );
    capture.argv.extend(["--".into(), "--port=5173".into()]);
    assert_eq!(
        capture.alternate(5174).unwrap().argv.last().unwrap(),
        "--port=5174"
    );
    fs::write(
        workspace.path.join("package.json"),
        r#"{"scripts":{"dev":"dangerous-command && vite"}}"#,
    )
    .unwrap();
    assert!(capture.alternate(5174).is_none());
}

#[test]
fn a_foreign_listener_never_counts_as_a_successful_retry_and_duplicate_launch_is_blocked() {
    let workspace = Workspace::new();
    let port = free_port();
    let mut capture = workspace.capture(port);
    capture.executable = "/bin/sleep".into();
    capture.argv = vec!["sleep".into(), "30".into()];
    let engine = Engine::new();
    let id = engine.0.register(capture).unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    let outsider = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        let _listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
        let _ = wait.recv_timeout(Duration::from_secs(25));
    });
    engine.0.act(&id, Action::Retry, false, None).unwrap();
    assert_eq!(view(&engine.0, &id).status, Status::Unverified);
    assert!(engine
        .0
        .act(&id, Action::Retry, true, None)
        .unwrap_err()
        .contains("still running"));
    release.send(()).unwrap();
    outsider.join().unwrap();
}

#[test]
fn previously_observed_same_project_development_server_can_use_one_click_recovery() {
    let workspace = Workspace::new();
    let port = free_port();
    let mut capture = workspace.capture(port);
    capture.argv[0] = "vite".into();
    let _owner = Owner::new(&capture, false);
    let engine = Engine::new();
    engine.0.scanner.lock().unwrap().scan().unwrap();
    let id = engine.0.register(capture).unwrap();
    assert_eq!(view(&engine.0, &id).safety.risk, Risk::Low);
    engine.0.act(&id, Action::KillRetry, false, None).unwrap();
    assert_eq!(view(&engine.0, &id).status, Status::Resolved);
}
#[test]
fn mixed_tcp_udp_ownership_blocks_termination() {
    let workspace = Workspace::new();
    let port = free_port();
    let capture = workspace.capture(port);
    let mut owner = Owner::new(&capture, false);
    let _udp = std::net::UdpSocket::bind(("127.0.0.1", port)).unwrap();
    let engine = Engine::new();
    let id = engine.0.register(capture).unwrap();
    assert_eq!(view(&engine.0, &id).safety.risk, Risk::Blocked);
    assert!(engine.0.act(&id, Action::KillRetry, true, None).is_err());
    assert!(owner.0.try_wait().unwrap().is_none());
}
