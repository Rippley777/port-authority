//! Display-only redaction. Never modifies executable launch arguments.
use crate::autopilot::capture::Capture;
use std::sync::LazyLock;
pub const MASK: &str = "••••••••";

pub fn sensitive(name: &str) -> bool {
    let name = name
        .trim_start_matches('-')
        .to_ascii_uppercase()
        .replace('-', "_");
    let words: Vec<_> = name.split('_').collect();
    words.iter().any(|part| {
        matches!(
            *part,
            "TOKEN"
                | "SECRET"
                | "PASSWORD"
                | "PASSWD"
                | "CREDENTIAL"
                | "CREDENTIALS"
                | "AUTH"
                | "SESSION"
                | "COOKIE"
        )
    }) || ["API_KEY", "PRIVATE_KEY", "ACCESS_KEY", "DATABASE_URL"]
        .iter()
        .any(|key| name == *key || name.ends_with(&format!("_{key}")))
        || matches!(
            name.as_str(),
            "KEY" | "PGPASSWORD" | "MYSQL_PWD" | "AUTHORIZATION"
        )
}
// Contextual assignments also cover JSON and explicit shell-command arguments.
static ASSIGNMENT: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)([a-z_][\w-]*)(["']?\s*[=:]\s*)("[^"]*"|'[^']*'|[^\s,;}]+)"#).unwrap()
});
static OPTION_VALUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(--[a-z][\w-]*)(\s+)("[^"]*"|'[^']*'|[^\s;|&]+)"#).unwrap()
});
static URL_VALUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?:https?|postgres(?:ql)?|mysql|redis)://[^\s'"<>]+"#).unwrap()
});
fn assignments(text: &str) -> String {
    let text = URL_VALUE.replace_all(text, |c: &regex::Captures| url_value(&c[0]));
    let text = OPTION_VALUE.replace_all(&text, |c: &regex::Captures| {
        if sensitive(&c[1]) {
            format!("{}{}{MASK}", &c[1], &c[2])
        } else {
            c[0].to_owned()
        }
    });
    ASSIGNMENT
        .replace_all(&text, |c: &regex::Captures| {
            if sensitive(&c[1]) {
                format!("{}{}{MASK}", &c[1], &c[2])
            } else {
                c[0].to_owned()
            }
        })
        .into_owned()
}
pub fn environment(name: &str, value: &str) -> Option<String> {
    (!sensitive(name)).then(|| assignments(value))
}
fn url_value(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return value.into();
    };
    if !matches!(
        url.scheme(),
        "http" | "https" | "postgres" | "postgresql" | "mysql" | "redis"
    ) {
        return value.into();
    }
    let mut changed = false;
    if url.password().is_some() {
        let _ = url.set_password(Some("REDACTED"));
        changed = true;
    }
    let pairs: Vec<_> = url
        .query_pairs()
        .map(|(k, v)| {
            if sensitive(&k) {
                changed = true;
                (k.into_owned(), "REDACTED".into())
            } else {
                (k.into_owned(), v.into_owned())
            }
        })
        .collect();
    if !changed {
        return value.into();
    }
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.to_string()
}
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub index: usize,
    pub classification: &'static str,
}
pub fn arguments(capture: &Capture) -> (Vec<String>, Vec<Decision>) {
    let mut next_secret = false;
    let mut decisions = vec![];
    let args = capture
        .argv
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let mut classification = "safe";
            let value = if next_secret {
                classification = "sensitive_option_value";
                MASK.into()
            } else if let Some((name, _)) = arg.split_once('=').filter(|(name, _)| sensitive(name))
            {
                classification = "sensitive_assignment";
                format!("{name}={MASK}")
            } else {
                let value = assignments(&url_value(arg));
                if value != *arg {
                    classification = "credential_in_value";
                }
                // An entire long argument matching a named secret is evidence. Never
                // replace a short value or a substring inside a path/version/option.
                if capture
                    .env
                    .iter()
                    .any(|(k, v)| sensitive(k) && v.len() >= 8 && v == arg)
                {
                    classification = "exact_secret_argument";
                    MASK.into()
                } else {
                    value
                }
            };
            next_secret = arg.starts_with('-') && !arg.contains('=') && sensitive(arg);
            decisions.push(Decision {
                index,
                classification,
            });
            value
        })
        .collect();
    (args, decisions)
}
/// Unstructured output has no argv boundaries. Mask contextual assignments and
/// whole credential tokens only, never arbitrary environment substrings.
pub fn output(text: &str, capture: &Capture) -> String {
    let mut result = assignments(&url_value(text));
    let mut secrets: Vec<&str> = capture
        .env
        .iter()
        .filter(|(name, _)| sensitive(name))
        .flat_map(|(_, value)| std::iter::once(value.as_str()).chain(value.lines()))
        .filter(|value| value.len() >= 4)
        .collect();
    let safe = arguments(capture).0;
    for (raw, public) in capture.argv.iter().zip(&safe) {
        if raw != public {
            let value = raw.split_once('=').map(|(_, value)| value).unwrap_or(raw);
            if value.len() >= 4 {
                secrets.push(value);
            }
        }
    }
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    for value in secrets {
        let mut start = 0;
        let mut out = String::new();
        for (at, _) in result.match_indices(value) {
            let end = at + value.len();
            let boundary = |c: char| !c.is_alphanumeric() && !"_./-".contains(c);
            if (at == 0 || result[..at].chars().next_back().is_some_and(boundary))
                && (end == result.len() || result[end..].chars().next().is_some_and(boundary))
            {
                out.push_str(&result[start..at]);
                out.push_str(MASK);
                start = end;
            }
        }
        out.push_str(&result[start..]);
        result = out;
    }
    result
}
