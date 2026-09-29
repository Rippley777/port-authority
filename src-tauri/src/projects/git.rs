use super::{models::RepositoryInfo, repository::parse_remote};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// Isolated from the socket scan. Bounded output, deadline, no hooks, no network.
pub fn output(root: &Path, args: &[&str]) -> Option<String> {
    let mut child = Command::new("git")
        .arg("--no-optional-locks")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .unwrap_or_default(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = (&mut stdout).take(65537).read_to_end(&mut bytes);
        bytes
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            _ if start.elapsed() > Duration::from_millis(1200) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            _ => thread::sleep(Duration::from_millis(10)),
        }
    };
    let bytes = reader.join().ok()?;
    if !status?.success() || bytes.len() > 65536 {
        return None;
    }
    String::from_utf8(bytes).ok().map(|s| s.trim().to_string())
}
#[derive(Default)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    pub repository: Option<RepositoryInfo>,
    pub root: Option<String>,
}
pub fn resolve(root: &Path) -> GitInfo {
    let Some(git_root) = output(root, &["rev-parse", "--show-toplevel"]) else {
        return GitInfo::default();
    };
    let branch = output(root, &["symbolic-ref", "--short", "HEAD"]).or_else(|| {
        output(root, &["rev-parse", "--short", "HEAD"]).map(|s| format!("Detached · {s}"))
    });
    let remotes = output(
        root,
        &[
            "config",
            "--local",
            "--no-includes",
            "--get-regexp",
            "^remote\\..*\\.url$",
        ],
    )
    .unwrap_or_default();
    let mut choices: Vec<_> = remotes
        .lines()
        .filter_map(|l| l.split_once(char::is_whitespace))
        .collect();
    choices.sort_by_key(|(key, _)| (*key != "remote.origin.url", key.to_string()));
    let repository = choices.iter().find_map(|(_, v)| parse_remote(v));
    // Git status can run clean/process filters from repository configuration.
    // Disable every configured driver before inspecting tracked changes.
    let dirty = output(root, &["config", "--null", "--list"]).and_then(|config| {
        let mut args: Vec<String> = Vec::new();
        for record in config.split('\0') {
            let key = record.split('\n').next().unwrap_or_default();
            if key.starts_with("filter.")
                && [".clean", ".process", ".required"]
                    .iter()
                    .any(|end| key.ends_with(end))
            {
                args.push("-c".into());
                args.push(format!(
                    "{key}={}",
                    if key.ends_with(".required") {
                        "false"
                    } else {
                        ""
                    }
                ));
            }
        }
        args.extend(
            [
                "status",
                "--porcelain=v1",
                "--untracked-files=no",
                "--ignore-submodules=all",
            ]
            .map(str::to_string),
        );
        output(root, &args.iter().map(String::as_str).collect::<Vec<_>>()).map(|s| !s.is_empty())
    });

    GitInfo {
        branch,
        dirty,
        repository,
        root: Some(git_root),
    }
}
