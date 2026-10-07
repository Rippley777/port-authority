use super::models::*;
use crate::{
    autopilot::{capture::Capture, models::now},
    process::ProcessIdentity,
};

pub fn public_env(name: &str) -> bool {
    matches!(
        name,
        "NVM_INC"
            | "NVM_BIN"
            | "NVM_DIR"
            | "CARGO_HOME"
            | "RUSTUP_HOME"
            | "PATH"
            | "HOME"
            | "LANG"
            | "LC_ALL"
            | "NODE_ENV"
            | "PORT"
            | "VIRTUAL_ENV"
            | "RUST_LOG"
            | "SHELL"
            | "TERM"
            | "PWD"
            | "OLDPWD"
            | "USER"
            | "LOGNAME"
            | "TMPDIR"
            | "TMP"
            | "TEMP"
            | "SHLVL"
            | "_"
            | "COLORTERM"
            | "TERM_PROGRAM"
            | "TERM_PROGRAM_VERSION"
    ) && !sensitive(name)
}
pub use super::sanitizer::sensitive;
pub fn redact(text: &str, capture: &Capture) -> String {
    super::sanitizer::output(text, capture)
}
pub fn display(argv: &[String]) -> String {
    argv.iter()
        .map(|s| {
            if !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c))
            {
                s.clone()
            } else {
                format!("'{}'", s.replace('\'', "'\"'\"'"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn safe_argv(capture: &Capture) -> Vec<String> {
    super::sanitizer::arguments(capture).0
}

/// Explicit service configuration distinguishes the public dev server from
/// framework workers' transient control sockets. This never builds a command.
pub fn configured_ports(capture: &Capture) -> Vec<u16> {
    fn options(words: &[&str]) -> Vec<u16> {
        words
            .iter()
            .enumerate()
            .filter_map(|(index, word)| {
                word.strip_prefix("--port=")
                    .or_else(|| {
                        matches!(*word, "--port" | "-p")
                            .then(|| words.get(index + 1).copied())
                            .flatten()
                    })
                    .and_then(|value| value.parse::<u16>().ok())
                    .filter(|port| *port > 0)
            })
            .collect()
    }
    let args: Vec<_> = capture.argv.iter().map(String::as_str).collect();
    let explicit = options(&args);
    if !explicit.is_empty() {
        return explicit;
    }
    let environment_ports: Vec<u16> = capture
        .env
        .get("PORT")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .into_iter()
        .collect();
    let tool = std::path::Path::new(args.first().copied().unwrap_or_default())
        .file_name()
        .and_then(|s| s.to_str());
    let script = match tool {
        Some("npm" | "pnpm" | "yarn" | "bun") if args.get(1) == Some(&"run") => args.get(2),
        Some("pnpm" | "yarn" | "bun") => args.get(1),
        _ => None,
    };
    let Some(script) = script else {
        return environment_ports;
    };
    let path = std::path::Path::new(&capture.cwd).join("package.json");
    let Some(package) = crate::projects::manifests::read_bounded(&path, 1_048_576)
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return environment_ports;
    };
    let Some(script) = package
        .get("scripts")
        .and_then(|scripts| scripts.get(*script))
        .and_then(|s| s.as_str())
    else {
        return environment_ports;
    };
    if script.contains([';', '|', '&', '$', '`', '\n']) {
        return environment_ports;
    }
    let words: Vec<_> = script.split_whitespace().collect();
    if !matches!(words.first(), Some(&"next" | &"vite")) {
        return environment_ports;
    }
    let ports = options(&words);
    if ports.is_empty() {
        environment_ports
    } else {
        ports
    }
}
pub fn from_capture(c: &Capture, root: ProcessIdentity, source: Source) -> DisplayLaunchContext {
    let argv = safe_argv(c);
    let shell = std::path::Path::new(&c.executable)
        .file_name()
        .and_then(|p| p.to_str())
        .filter(|p| {
            matches!(
                *p,
                "sh" | "bash" | "zsh" | "fish" | "cmd.exe" | "powershell.exe" | "pwsh"
            )
        })
        .map(str::to_owned);
    let confidence = match source {
        Source::ShellObserved | Source::UserDefined => Confidence::High,
        Source::ParentProcess => Confidence::Medium,
        _ => Confidence::Low,
    };
    let mut context = DisplayLaunchContext {
        schema_version: 2,
        run_id: None,
        environment_strategy: Some("current_with_captured_toolchain".into()),
        recovery_confidence: match source {
            Source::UserDefined => RecoveryConfidence::Exact,
            Source::ShellObserved => RecoveryConfidence::Observed,
            _ => RecoveryConfidence::Recovered,
        },
        id: uuid::Uuid::new_v4().to_string(),
        source,
        confidence,
        kind: if shell.is_some() {
            LaunchKind::ShellCommand
        } else {
            LaunchKind::DirectProcess
        },
        command: display(&argv),
        executable: c.executable.clone(),
        args: argv.into_iter().skip(1).collect(),
        working_directory: c.cwd.clone(),
        environment: c
            .env
            .iter()
            .map(|(k, v)| EnvironmentVariable {
                name: k.clone(),
                value: super::sanitizer::environment(k, v),
            })
            .collect(),
        shell,
        project_id: None,
        launch_root: root,
        captured_at: now(),
        recoverable: true,
        reason: String::new(),
        relaunched_at: None,
        fingerprint: String::new(),
    };
    context.fingerprint = context.grouping_key();
    context
}

pub fn command(capture: &Capture) -> Result<std::process::Command, String> {
    use std::process::{Command, Stdio};
    capture.validate()?;
    if capture.argv.iter().any(|arg| arg.contains("••••"))
        || capture.executable.contains("••••")
        || capture.cwd.contains("••••")
    {
        return Err(
            "Masked display text cannot be executed. Capture the original command again.".into(),
        );
    }
    let mut command = Command::new(&capture.executable);
    command
        .args(&capture.argv[1..])
        .current_dir(&capture.cwd)
        .env_clear()
        .envs(&capture.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.arg0(&capture.argv[0]).process_group(0);
    }
    Ok(command)
}

/// Redact credential-shaped arguments on every frontend/timeline serialization,
/// including historical ancestor snapshots which have no live environment.
pub fn serialize_argv<S: serde::Serializer>(
    argv: &[String],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    let capture = Capture {
        argv: argv.to_vec(),
        executable: String::new(),
        cwd: String::new(),
        env: Default::default(),
        error: String::new(),
        exit_code: 1,
    };
    safe_argv(&capture).serialize(serializer)
}
