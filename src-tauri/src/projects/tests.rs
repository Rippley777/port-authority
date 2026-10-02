use super::*;
use crate::projects::{manifests, repository, resolver};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("pa-project-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn write(&self, path: &str, content: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    fn git(&self, args: &[&str]) {
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn entry(cwd: Option<&Path>) -> PortEntry {
    serde_json::from_value(serde_json::json!({"id":"test","port":5173,"protocol":"TCP","address":"127.0.0.1","pid":12345,"process":"node","command":["node","server.js"],"executable":"/usr/bin/node","cwd":cwd.map(|p|p.to_string_lossy()),"parentPid":900,"user":"developer","startedAt":1000,"memory":null,"cpu":null,"system":false,"protected":false,"restartable":false,"restartReason":"test","permissionLimited":false})).unwrap()
}
#[test]
fn walks_nested_monorepos_and_skips_dependencies() {
    let f = Fixture::new();
    f.write(".git/HEAD", "ref: refs/heads/main");
    f.write("package.json", r#"{"name":"workspace"}"#);
    f.write(
        "apps/web/package.json",
        r#"{"name":"@acme/shipwreck","dependencies":{"vite":"*","react":"*"}}"#,
    );
    f.write("apps/web/src/deep/file.js", "");
    f.write(
        "apps/web/node_modules/vite/package.json",
        r#"{"name":"vite"}"#,
    );
    for path in ["apps/web/src/deep", "apps/web/node_modules/vite"] {
        let (root, markers) = resolver::find_root(&f.0.join(path)).unwrap();
        assert_eq!(root, f.0.join("apps/web"));
        let m = manifests::resolve(&root, &markers);
        assert_eq!(m.name.as_deref(), Some("@acme/shipwreck"));
        assert_eq!(m.frameworks, vec!["Vite", "React"]);
    }
    assert_eq!(
        manifests::display_name("@acme/port-authority"),
        "Port Authority"
    );
}
#[test]
fn manifests_handle_cargo_python_malformed_and_missing() {
    for (filename, content, name, kind) in [
        (
            "Cargo.toml",
            "[package]\nname='rust-api'",
            "rust-api",
            "Rust",
        ),
        (
            "pyproject.toml",
            "[project]\nname='plant-journal'\ndependencies=['fastapi']",
            "plant-journal",
            "Python",
        ),
    ] {
        let f = Fixture::new();
        f.write(filename, content);
        let m = manifests::resolve(&f.0, &[filename.into()]);
        assert_eq!(m.name.as_deref(), Some(name));
        assert_eq!(m.ecosystem.as_deref(), Some(kind));
    }
    let f = Fixture::new();
    f.write("package.json", "{broken");
    assert!(manifests::resolve(&f.0, &["package.json".into()])
        .name
        .is_none());
    assert!(resolver::find_root(&f.0.join("removed")).is_none());
    f.write("large", &"x".repeat(256 * 1024 + 1));
    assert!(manifests::read_small(&f.0.join("large")).is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = f.0.join("package.json");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
        if unsafe { libc::geteuid() } != 0 {
            assert!(manifests::read_small(&path).is_none());
        }
    }
}
#[test]
fn parses_remotes_without_leaking_secrets_or_guessing_private_web_hosts() {
    for value in [
        "git@github.com:Rippley777/shipwreck.git",
        "https://token:secret@github.com/Rippley777/shipwreck.git?token=secret#fragment",
        "ssh://git@github.com/Rippley777/shipwreck.git",
    ] {
        let r = repository::parse_remote(value).unwrap();
        assert_eq!(r.owner.as_deref(), Some("Rippley777"));
        assert_eq!(
            r.web_url.as_deref(),
            Some("https://github.com/Rippley777/shipwreck")
        );
        assert!(!r.remote_url.contains("secret"));
    }
    assert!(
        repository::parse_remote("git@internal.example:team/sub/project.git")
            .unwrap()
            .web_url
            .is_none()
    );
    assert_eq!(
        repository::parse_remote("https://git.example/team/project.git")
            .unwrap()
            .web_url
            .as_deref(),
        Some("https://git.example/team/project")
    );
    for value in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "git@github.com:a/../secret",
        "https://github.com/a/%0aevil",
        "/tmp/local.git",
    ] {
        assert!(repository::parse_remote(value).is_none());
    }
}
#[test]
fn git_metadata_handles_multiple_remotes_worktrees_and_tracked_changes() {
    let f = Fixture::new();
    f.git(&["init", "-b", "main"]);
    f.write("package.json", r#"{"name":"shipwreck"}"#);
    f.git(&["add", "package.json"]);
    f.git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-m",
        "fixture",
    ]);
    f.git(&[
        "remote",
        "add",
        "upstream",
        "git@gitlab.com:other/project.git",
    ]);
    f.git(&[
        "remote",
        "add",
        "origin",
        "git@github.com:owner/shipwreck.git",
    ]);
    let info = crate::projects::git::resolve(&f.0);
    assert_eq!(info.branch.as_deref(), Some("main"));
    assert_eq!(info.dirty, Some(false));
    assert_eq!(info.repository.unwrap().owner.as_deref(), Some("owner"));
    f.git(&["worktree", "add", "-b", "feature", "worktree"]);
    let (root, markers) = resolver::find_root(&f.0.join("worktree")).unwrap();
    assert_eq!(root, f.0.join("worktree"));
    let identity = resolver::identity(&root, &markers);
    assert_eq!(identity.git_branch.as_deref(), Some("feature"));
    assert_eq!(identity.repository.unwrap().repository, "shipwreck");
    f.write("package.json", r#"{"name":"changed"}"#);
    assert_eq!(crate::projects::git::resolve(&f.0).dirty, Some(true));
}
#[test]
fn child_cwd_wins_parent_fallback_is_tentative_and_infrastructure_is_not_inferred() {
    let child = Fixture::new();
    child.write("Cargo.toml", "[package]\nname='child'");
    let parent = Fixture::new();
    parent.write("package.json", r#"{"name":"parent"}"#);
    assert_eq!(
        resolver::candidate(&entry(Some(&child.0)), Some(&parent.0))
            .unwrap()
            .0,
        child.0
    );
    assert!(resolver::candidate(&entry(None), Some(&parent.0)).is_none());
    let mut infra = entry(None);
    infra.process = "redis-server".into();
    infra.command = vec!["redis-server".into()];
    assert!(resolver::candidate(&infra, Some(&parent.0)).is_none());
    assert!(resolver::candidate(&entry(None), None).is_none());
}
#[test]
fn cache_is_pid_start_sensitive_reuses_metadata_and_prunes_disappeared_processes() {
    let f = Fixture::new();
    f.write("package.json", r#"{"name":"shipwreck"}"#);
    let engine = ProjectEngine::new(None);
    let p = entry(Some(&f.0));
    engine.resolve_entries(std::slice::from_ref(&p));
    f.write("package.json", r#"{"name":"changed"}"#);
    engine.resolve_entries(std::slice::from_ref(&p));
    let mut ports = vec![p.clone()];
    engine.enrich(&mut ports, false);
    assert_eq!(ports[0].project.as_ref().unwrap().name, "Shipwreck");
    ports[0].started_at = Some(2000);
    engine.enrich(&mut ports, false);
    assert!(ports[0].project.is_none());
    engine.invalidate();
    engine.resolve_entries(std::slice::from_ref(&p));
    engine.enrich(&mut ports, false);
    assert!(ports[0].project.is_none());
    let mut original = vec![p];
    engine.enrich(&mut original, false);
    assert_eq!(original[0].project.as_ref().unwrap().name, "Changed");
    engine.resolve_entries(&[]);
    engine.enrich(&mut original, false);
    assert!(original[0].project.is_none());
    assert_eq!(engine.snapshot().projects.len(), 1);
}
#[test]
fn pins_and_recent_metadata_survive_restart_without_source_content() {
    let f = Fixture::new();
    f.write(
        "package.json",
        r#"{"name":"shipwreck","secret":"DO_NOT_STORE"}"#,
    );
    let path = f.0.join("state/recent.json");
    let engine = ProjectEngine::new(Some(path.clone()));
    engine.resolve_entries(&[entry(Some(&f.0))]);
    engine.pin(f.0.to_str().unwrap(), true).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("DO_NOT_STORE"));
    let restored = ProjectEngine::new(Some(path));
    assert!(restored.snapshot().projects[0].pinned);
}

#[test]
fn metadata_never_runs_repository_clean_filters() {
    let f = Fixture::new();
    f.git(&["init", "-b", "main"]);
    f.write("package.json", r#"{"name":"safe"}"#);
    f.write(".gitattributes", "package.json filter=custom");
    f.git(&["add", "."]);
    f.git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-m",
        "fixture",
    ]);
    f.git(&[
        "config",
        "filter.custom.clean",
        "touch FILTER_EXECUTED; cat",
    ]);
    f.write("package.json", r#"{"name":"changed"}"#);
    let info = crate::projects::git::resolve(&f.0);
    assert_eq!(info.dirty, Some(true));
    assert!(!f.0.join("FILTER_EXECUTED").exists());
}

#[test]
fn custom_editor_receives_literal_project_path_without_shell_evaluation() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new();
        let root = f.0.join("spaces; $(not-a-command)");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("package.json"), r#"{"name":"literal"}"#).unwrap();
        let engine = ProjectEngine::new(None);
        engine.resolve_entries(&[entry(Some(&root))]);
        let stub = f.0.join("editor");
        std::fs::write(
            &stub,
            "#!/bin/sh\nprintf '%s' \"$1\" > \"$1/received-path\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o700)).unwrap();
        crate::projects::actions::act(
            &engine,
            root.to_str().unwrap(),
            "editor",
            "custom",
            stub.to_str(),
        )
        .unwrap();
        let start = Instant::now();
        while !root.join("received-path").exists() && start.elapsed() < Duration::from_secs(2) {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            std::fs::read_to_string(root.join("received-path")).unwrap(),
            root.to_string_lossy()
        );
        assert!(crate::projects::actions::act(
            &engine,
            "/unobserved/project",
            "editor",
            "custom",
            stub.to_str()
        )
        .is_err());
        std::fs::remove_dir_all(&root).unwrap();
        assert!(crate::projects::actions::act(
            &engine,
            root.to_str().unwrap(),
            "editor",
            "custom",
            stub.to_str()
        )
        .is_err());
    }
}

#[test]
fn enrichment_does_not_wait_for_git_and_resolves_a_real_listener_asynchronously() {
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tcp.local_addr().unwrap().port();
    let mut entries = crate::ports::scanner::Scanner::new().scan().unwrap();
    let engine = ProjectEngine::new(None);
    let start = Instant::now();
    engine.enrich(&mut entries, true);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(entries
        .iter()
        .find(|p| p.port == port)
        .unwrap()
        .project
        .is_none());
    let deadline = Instant::now();
    loop {
        engine.enrich(&mut entries, false);
        if entries
            .iter()
            .find(|p| p.port == port)
            .unwrap()
            .project
            .is_some()
        {
            break;
        }
        assert!(deadline.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        entries
            .iter()
            .find(|p| p.port == port)
            .unwrap()
            .project
            .as_ref()
            .unwrap()
            .name,
        "Port Authority"
    );
}

#[test]
fn idle_project_metadata_can_refresh_without_claiming_it_is_running() {
    let f = Fixture::new();
    f.write("package.json", r#"{"name":"before"}"#);
    let engine = ProjectEngine::new(None);
    engine.resolve_entries(&[entry(Some(&f.0))]);
    engine.resolve_entries(&[]);
    let observed = engine.snapshot().projects[0].last_observed;
    f.write("package.json", r#"{"name":"after"}"#);
    assert_eq!(
        engine.refresh_project(f.0.to_str().unwrap()).unwrap().name,
        "After"
    );
    assert_eq!(engine.snapshot().projects[0].last_observed, observed);
}
