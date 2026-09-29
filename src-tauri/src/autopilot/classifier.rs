use super::models::{now, Category, Risk, Safety};
use crate::ports::models::PortEntry;
use std::path::{Path, PathBuf};

pub fn project_root(cwd: &str) -> PathBuf {
    let original = Path::new(cwd)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(cwd));
    for path in original.ancestors().take(12) {
        if [
            "package.json",
            ".git",
            "Cargo.toml",
            "pyproject.toml",
            "go.mod",
        ]
        .iter()
        .any(|name| path.join(name).exists())
        {
            return path.to_path_buf();
        }
    }
    original
}
pub fn project_name(cwd: &str) -> String {
    project_root(cwd)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_owned())
}

pub fn classify(owner: Option<&PortEntry>, failed_cwd: &str, previously_observed: bool) -> Safety {
    let mut safety = Safety {
        category: Category::Unknown,
        risk: Risk::Blocked,
        heading: "Owner could not be verified".into(),
        reasons: vec![],
        project: None,
        project_path: None,
        different_project: false,
        previously_observed,
        possibly_stale: false,
    };
    let Some(p) = owner else {
        safety.reasons.push(
            "No single, identifiable TCP owner was found. Nothing will be terminated.".into(),
        );
        return safety;
    };
    safety.project = p.cwd.as_deref().map(project_name);
    safety.project_path = p
        .cwd
        .as_deref()
        .map(|d| project_root(d).to_string_lossy().into_owned());
    safety.different_project = p
        .cwd
        .as_deref()
        .is_some_and(|d| project_root(d) != project_root(failed_cwd));
    if p.protected
        || p.system
        || p.user.as_deref() == Some("root")
        || p.pid.is_none_or(|pid| pid <= 4)
    {
        safety.category = Category::SystemProcess;
        safety.heading = "Protected process".into();
        safety.reasons.push("This is a critical, privileged, different-user, or application-owned process. Autopilot cannot stop it.".into());
        return safety;
    }
    if p.started_at.is_none() || p.executable.is_none() || p.user.is_none() || p.command.is_empty()
    {
        safety.reasons.push("Executable, command, user, and start time must be available before process control is allowed.".into());
        return safety;
    }
    // Inspect process/executable identity and actual argument basenames, never port numbers.
    let identities: Vec<String> = std::iter::once(p.process.as_str())
        .chain(p.executable.as_deref())
        .chain(p.command.iter().map(String::as_str))
        .filter_map(|s| Path::new(s).file_name())
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .collect();
    let infrastructure = identities.iter().any(|n| {
        matches!(
            n.as_str(),
            "postgres"
                | "postgresql"
                | "mysqld"
                | "mysql"
                | "mariadbd"
                | "redis-server"
                | "redis"
                | "mongod"
                | "docker"
                | "dockerd"
                | "com.docker.backend"
                | "containerd"
                | "sshd"
                | "nginx"
                | "rabbitmq-server"
                | "kubelet"
        )
    });
    let critical = identities.iter().any(|n| {
        matches!(
            n.as_str(),
            "systemd" | "launchd" | "init" | "sshd" | "lsass.exe" | "svchost.exe"
        )
    });
    if critical {
        safety.category = Category::SystemProcess;
        safety.heading = "Potentially critical service".into();
        safety
            .reasons
            .push("Autopilot will not terminate this service.".into());
        return safety;
    }
    let development = identities.iter().any(|n| {
        matches!(
            n.as_str(),
            "vite"
                | "vite.js"
                | "next"
                | "next-server"
                | "webpack"
                | "webpack-dev-server"
                | "react-scripts"
                | "nodemon"
                | "uvicorn"
                | "flask"
        )
    }) || (identities.iter().any(|n| n == "node" || n == "python3")
        && p.command
            .iter()
            .any(|s| matches!(s.as_str(), "dev" | "http.server" | "--watch")));
    if infrastructure {
        safety.category = Category::Infrastructure;
        safety.risk = Risk::High;
        safety.heading = "Caution: infrastructure service".into();
        safety.reasons.push(format!("{} appears to be a database or infrastructure service. Stopping it may interrupt other applications. Explicit confirmation is required.",p.process));
    } else if development {
        safety.category = Category::DevServer;
        safety.risk = Risk::Low;
        safety.heading = "Looks like a development server".into();
        safety.reasons.push("Identified from the executable and command arguments. This is a heuristic, not a guarantee that stopping it is harmless.".into());
    } else {
        safety.category = if p.cwd.is_some() {
            Category::UserProcess
        } else {
            Category::Unknown
        };
        safety.risk = Risk::High;
        safety.heading = "Review this process before stopping it".into();
        safety.reasons.push("The command is not a recognized development server. Its purpose cannot be established with confidence.".into());
    }
    if safety.different_project {
        if safety.risk == Risk::Low {
            safety.risk = Risk::Medium;
        }
        safety.reasons.push("The owner belongs to a different project. Confirm that it is safe to interrupt that work.".into());
    }
    if !previously_observed {
        if safety.risk == Risk::Low {
            safety.risk = Risk::Medium;
        }
        safety
            .reasons
            .push("Port Authority had not observed this process before this conflict.".into());
    }
    if p.cwd.is_none() || p.parent_pid.is_none() {
        safety.risk = Risk::High;
        safety.reasons.push("Working directory or parent process information is unavailable; project attribution is incomplete.".into());
    }
    safety.possibly_stale = safety.category == Category::DevServer
        && safety.different_project
        && p.started_at
            .is_some_and(|start| now().saturating_sub(start) >= 3 * 3600);
    if safety.possibly_stale {
        safety.reasons.push("Possibly left over: this development server is over three hours old and belongs to another project. Age alone does not establish that a process is stale.".into());
    }
    safety
}
