use std::path::Path;
/// Adapters select only intact argv, never parse rewritten process titles or infer a package manager.
pub fn supported(argv: &[String]) -> bool {
    let Some(first) = argv
        .first()
        .and_then(|s| Path::new(s).file_name())
        .and_then(|s| s.to_str())
    else {
        return false;
    };
    match first {
        "npm" | "pnpm" | "yarn" | "bun" => argv.len() > 1,
        "cargo" => {
            argv.get(1).is_some_and(|s| s == "run")
                || (argv.get(1).is_some_and(|s| s == "tauri")
                    && argv.get(2).is_some_and(|s| s == "dev"))
        }
        s if s.starts_with("python") => argv.len() > 1,
        "node" | "nodejs" => argv.get(1).is_some_and(|s| {
            ["npm-cli.js", "pnpm.cjs", "yarn.js", "yarn.cjs"]
                .iter()
                .any(|name| s.ends_with(name))
        }),
        _ => false,
    }
}
pub fn excluded(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "docker",
        "containerd",
        "com.docker",
        "podman",
        "launchd",
        "systemd",
        "supervisord",
        "pm2",
    ]
    .iter()
    .any(|v| name.contains(v))
}

pub fn launch_root(name: &str, argv: &[String]) -> bool {
    supported(argv)
        || ["npm", "pnpm", "yarn", "bun", "cargo"]
            .iter()
            .any(|tool| name == *tool || name.starts_with(&format!("{tool} ")))
}

/// An argv headed by npm cannot be replayed directly with an underlying Node
/// executable unless the npm CLI script itself is still present in argv.
pub fn compatible(argv: &[String], executable: &Path) -> bool {
    let arg = argv
        .first()
        .and_then(|a| Path::new(a).file_name())
        .and_then(|a| a.to_str());
    let exe = executable.file_name().and_then(|a| a.to_str());
    match (arg, exe) {
        (Some(arg), Some(exe)) => {
            arg == exe || (arg.starts_with("python") && exe.starts_with("python"))
        }
        _ => false,
    }
}
