use super::cache::ProjectEngine;
use serde::Serialize;
use std::{path::Path, process::Command};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applications {
    pub editors: Vec<String>,
    pub terminals: Vec<String>,
}
fn executable(bin: &str) -> Option<std::path::PathBuf> {
    let names = if cfg!(windows) && !bin.ends_with(".exe") {
        vec![format!("{bin}.exe")]
    } else {
        vec![bin.to_string()]
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|p| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    p.metadata()
                        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    p.is_file()
                }
            })
    })
}
#[cfg(any(target_os = "linux", windows))]
fn available(bin: &str) -> bool {
    executable(bin).is_some()
}
fn editor_executable(bin: &str) -> Option<std::path::PathBuf> {
    if let Some(path) = executable(bin) {
        return Some(path);
    }
    #[cfg(windows)]
    {
        let relative = match bin {
            "code" => "Microsoft VS Code/Code.exe",
            "code-insiders" => "Microsoft VS Code Insiders/Code - Insiders.exe",
            "cursor" => "cursor/Cursor.exe",
            "subl" => "Sublime Text/sublime_text.exe",
            "zed" => "Zed/zed.exe",
            _ => return None,
        };
        for base in [
            std::env::var_os("LOCALAPPDATA").map(|p| Path::new(&p).join("Programs")),
            std::env::var_os("ProgramFiles").map(std::path::PathBuf::from),
        ]
        .into_iter()
        .flatten()
        {
            let path = base.join(relative);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn launch(mut command: Command) -> Result<(), String> {
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().map_err(|e| {
        format!(
            "Could not open application: {e}. Configure your preferred application in Settings."
        )
    })?;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(150) {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("Application could not open the project ({status}). Check your preferred application in Settings."))
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
pub fn applications() -> Applications {
    let mut editors = Vec::new();
    for (id, bin, app) in [
        ("vscode", "code", "Visual Studio Code"),
        ("insiders", "code-insiders", "Visual Studio Code - Insiders"),
        ("cursor", "cursor", "Cursor"),
        ("zed", "zed", "Zed"),
        ("sublime", "subl", "Sublime Text"),
        ("idea", "idea", "IntelliJ IDEA"),
        ("webstorm", "webstorm", "WebStorm"),
        ("pycharm", "pycharm", "PyCharm"),
    ] {
        if editor_executable(bin).is_some() || cfg!(target_os = "macos") && mac_app(app).is_some() {
            editors.push(id.into());
        }
    }
    let terminals = if cfg!(target_os = "macos") {
        [
            ("terminal", "Terminal"),
            ("iterm", "iTerm"),
            ("warp", "Warp"),
            ("ghostty", "Ghostty"),
        ]
        .iter()
        .filter(|(_, app)| mac_app(app).is_some() || *app == "Terminal")
        .map(|(id, _)| id.to_string())
        .collect()
    } else if cfg!(windows) {
        vec!["windows-terminal".into(), "powershell".into()]
    } else {
        vec!["system".into()]
    };
    Applications { editors, terminals }
}
fn mac_app(name: &str) -> Option<std::path::PathBuf> {
    [
        Some(std::path::PathBuf::from("/Applications")),
        std::env::var_os("HOME").map(|p| Path::new(&p).join("Applications")),
    ]
    .into_iter()
    .flatten()
    .map(|p| p.join(format!("{name}.app")))
    .find(|p| p.is_dir())
}
pub fn act(
    engine: &ProjectEngine,
    root: &str,
    action: &str,
    preference: &str,
    custom: Option<&str>,
) -> Result<(), String> {
    let identity = engine.known(root)?;
    let canonical = Path::new(root)
        .canonicalize()
        .map_err(|_| "This project directory is no longer available.")?;
    if !canonical.is_dir() || canonical != Path::new(root) {
        return Err("Project location changed. Refresh project metadata before opening it.".into());
    }
    match action {
        "repository" => {
            let url = identity
                .repository
                .and_then(|r| r.web_url)
                .ok_or("A safe repository web URL is not available.")?;
            let parsed = url::Url::parse(&url).map_err(|_| "Invalid repository URL")?;
            if parsed.scheme() != "https"
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.host_str().is_none()
            {
                return Err("Unsupported repository URL".into());
            }
            open::that(url).map_err(|e| e.to_string())
        }
        "reveal" => {
            #[cfg(target_os = "macos")]
            {
                let mut c = Command::new("open");
                c.arg("-R").arg(&canonical);
                launch(c)
            }
            #[cfg(not(target_os = "macos"))]
            {
                open::that(&canonical).map_err(|e| e.to_string())
            }
        }
        "editor" => {
            let apps = applications();
            let preference = if preference == "auto" {
                apps.editors.first().map(String::as_str).ok_or(
                    "No supported editor was found. Configure your preferred editor in Settings.",
                )?
            } else {
                preference
            };
            if preference == "custom" {
                let exe = custom.filter(|s|Path::new(s).is_absolute() && Path::new(s).is_file()).ok_or("Choose an absolute executable path in Settings. Arguments and shell commands are not accepted.")?;
                #[cfg(windows)]
                if !Path::new(exe)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                {
                    return Err(
                        "Select an .exe editor. Batch files and shell commands are not supported."
                            .into(),
                    );
                }
                let mut c = Command::new(exe);
                c.arg(&canonical);
                return launch(c);
            }
            let (bin, app) = match preference {
                "vscode" => ("code", "Visual Studio Code"),
                "insiders" => ("code-insiders", "Visual Studio Code - Insiders"),
                "cursor" => ("cursor", "Cursor"),
                "zed" => ("zed", "Zed"),
                "sublime" => ("subl", "Sublime Text"),
                "idea" => ("idea", "IntelliJ IDEA"),
                "webstorm" => ("webstorm", "WebStorm"),
                "pycharm" => ("pycharm", "PyCharm"),
                _ => return Err("Unsupported editor preference".into()),
            };
            #[cfg(target_os = "macos")]
            if let Some(app) = mac_app(app) {
                let mut c = Command::new("open");
                c.arg("-a").arg(app).arg(&canonical);
                return launch(c);
            }
            let _ = app;
            let executable = editor_executable(bin).ok_or_else(|| {
                format!("{app} could not be found. Configure your preferred editor in Settings.")
            })?;
            let mut c = Command::new(executable);
            c.arg(&canonical);
            launch(c)
        }
        "terminal" => terminal(&canonical, preference),
        _ => Err("Unknown project action".into()),
    }
}
pub fn terminal(root: &Path, preference: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app = match preference {
            "auto" | "terminal" => "Terminal",
            "iterm" => "iTerm",
            "warp" => "Warp",
            "ghostty" => "Ghostty",
            _ => return Err("This terminal is not supported on macOS.".into()),
        };
        if app != "Terminal" && mac_app(app).is_none() {
            return Err(format!(
                "{app} could not be found. Configure your terminal in Settings."
            ));
        }
        if preference == "warp" {
            let mut uri = url::Url::parse("warp://action/new_window").map_err(|e| e.to_string())?;
            uri.query_pairs_mut()
                .append_pair("path", &root.to_string_lossy());
            let mut c = Command::new("open");
            c.arg("-a").arg("Warp").arg(uri.as_str());
            return launch(c);
        }
        // Finder's open-directory integration avoids shell/AppleScript interpolation.
        let mut c = Command::new("open");
        c.arg("-a").arg(app).arg(root);
        launch(c)
    }
    #[cfg(target_os = "linux")]
    {
        if !matches!(preference, "auto" | "system") {
            return Err("Select System terminal on Linux.".into());
        }
        for (bin, flag) in [
            ("x-terminal-emulator", None),
            ("gnome-terminal", Some("--working-directory")),
            ("konsole", Some("--workdir")),
            ("xterm", None),
        ] {
            if available(bin) {
                let mut c = Command::new(bin);
                c.current_dir(root);
                if let Some(flag) = flag {
                    c.arg(flag).arg(root);
                }
                return launch(c);
            }
        }
        Err("No supported system terminal was found.".into())
    }
    #[cfg(target_os = "windows")]
    {
        let bin = match preference {
            "auto" => {
                if available("wt.exe") {
                    "wt.exe"
                } else {
                    "powershell.exe"
                }
            }
            "windows-terminal" => "wt.exe",
            "powershell" => "powershell.exe",
            _ => return Err("Select a Windows terminal in Settings.".into()),
        };
        let mut c = Command::new(bin);
        c.current_dir(root);
        if bin == "wt.exe" {
            c.arg("-d").arg(root);
        } else {
            c.arg("-NoExit");
        }
        launch(c)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (root, preference);
        Err("Terminal integration is unavailable on this platform.".into())
    }
}
