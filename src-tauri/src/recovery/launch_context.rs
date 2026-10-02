use super::models::*;
use crate::{
    autopilot::{capture::Capture, models::now},
    process::ProcessIdentity,
};

pub fn public_env(name: &str) -> bool {
    matches!(
        name,
        "PATH"
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
pub fn sensitive(name: &str) -> bool {
    let upper = name.to_uppercase();
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASS",
        "KEY",
        "CREDENTIAL",
        "AUTH",
        "COOKIE",
        "SESSION",
        "DATABASE_URL",
        "API_URL",
    ]
    .iter()
    .any(|s| upper.contains(s))
}
pub fn redact(text: &str, capture: &Capture) -> String {
    let mut out = text.to_owned();
    let mut values: Vec<_> = capture
        .env
        .iter()
        .filter(|(k, v)| !public_env(k) && !v.is_empty())
        .flat_map(|(_, v)| {
            std::iter::once(v.as_str())
                .chain(v.lines())
                .filter(|v| !v.is_empty())
        })
        .collect();
    values.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for value in values {
        out = out.replace(value, "••••••••");
    }
    let mut secret_next = false;
    for arg in &capture.argv {
        if secret_next && !arg.is_empty() {
            out = out.replace(arg, "••••••••");
        }
        if let Some((name, value)) = arg.split_once('=') {
            if sensitive(name) && !value.is_empty() {
                out = out.replace(value, "••••••••");
            }
        }
        secret_next = arg.starts_with('-') && sensitive(arg) && !arg.contains('=');
    }
    // Arguments and output can contain credentials independent of the environment.
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"(?i)((?:[\w-]*(?:token|secret|password|credential|auth|cookie|session|key)[\w-]*|database_url|api_url)["']?\s*[=:]\s*)(?:"[^"]*"|'[^']*'|[^\s]+)"#).unwrap()
    });
    PATTERN.replace_all(&out, "${1}••••••••").into_owned()
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
    let mut secret_next = false;
    capture
        .argv
        .iter()
        .map(|a| {
            let value = if secret_next {
                "••••••••".into()
            } else {
                redact(a, capture)
            };
            secret_next = a.starts_with('-') && sensitive(a) && !a.contains('=');
            value
        })
        .collect()
}
pub fn from_capture(c: &Capture, root: ProcessIdentity, source: Source) -> LaunchContext {
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
    let mut context = LaunchContext {
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
                value: public_env(k).then(|| v.clone()),
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
    context.fingerprint = CommandFingerprint::from_context(&context).key();
    context
}

pub fn command(capture: &Capture) -> Result<std::process::Command, String> {
    use std::process::{Command, Stdio};
    capture.validate()?;
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
