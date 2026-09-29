use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};
static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap());
static CONFLICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)EADDRINUSE|address\s+already\s+in\s+use|port\s+\d+\s+(?:is\s+)?already\s+in\s+use",
    )
    .unwrap()
});
static PORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\bport\s*[:=]?\s*['\"]?(\d{1,5})\b|(?:\[[0-9a-f:%]+\]|(?:\d{1,3}\.){3}\d{1,3}|localhost|::):(\d{1,5})\b"#,
    )
    .unwrap()
});
/// Only a failed command with unambiguous conflict evidence is eligible. Never
/// infer a desired port solely from a framework's default port number.
pub fn detect(output: &str, failed: bool) -> Option<u16> {
    if !failed {
        return None;
    }
    let clean = ANSI.replace_all(output, "");
    // Node datagram errors are not evidence that a TCP owner should be stopped.
    if clean.contains("bind EADDRINUSE") || clean.to_lowercase().contains("udp") {
        return None;
    }
    let found = CONFLICT.find(&clean)?;
    let tail = &clean[found.start()..];
    let block: String = tail.lines().take(12).collect::<Vec<_>>().join("\n");
    let ports: BTreeSet<u16> = PORT
        .captures_iter(&block)
        .filter_map(|c| {
            c.get(1)
                .or_else(|| c.get(2))
                .and_then(|m| m.as_str().parse::<u16>().ok())
                .filter(|p| *p > 0)
        })
        .collect();
    (ports.len() == 1).then(|| *ports.first().unwrap())
}
pub fn clean_output(output: &str) -> String {
    ANSI.replace_all(output, "")
        .chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_only_unambiguous_failed_conflicts() {
        for (message, port) in [
            (
                "Error: listen EADDRINUSE: address already in use :::5173",
                5173,
            ),
            ("Error: listen EADDRINUSE 127.0.0.1:3000", 3000),
            ("EADDRINUSE\n  address: '::',\n  port: 8080", 8080),
            ("Port 5173 is already in use", 5173),
            ("\x1b[31mError: EADDRINUSE [::1]:5173\x1b[0m", 5173),
        ] {
            assert_eq!(detect(message, true), Some(port), "{message}");
        }
        for message in [
            "server 5173 failed",
            "Error: bind EADDRINUSE 0.0.0.0:5173",
            "UDP EADDRINUSE :::5173",
            "EADDRINUSE",
            "EADDRINUSE :::65536",
            "EADDRINUSE :::0",
            "EADDRINUSE :::5173\nport: 3000",
        ] {
            assert_eq!(detect(message, true), None);
        }
        assert_eq!(detect("Port 5173 is already in use", false), None);
    }
}
