use super::models::RepositoryInfo;

pub fn parse_remote(raw: &str) -> Option<RepositoryInfo> {
    let raw = raw.trim();
    if raw.len() > 2048 || raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let (host, path, http) =
        if raw.starts_with("https://") || raw.starts_with("http://") || raw.starts_with("ssh://") {
            let url = url::Url::parse(raw).ok()?;
            if !matches!(url.scheme(), "https" | "http" | "ssh") {
                return None;
            }
            (
                url.host_str()?.to_string(),
                url.path().trim_start_matches('/').to_string(),
                url.scheme() == "https" && url.port().is_none(),
            )
        } else {
            if raw.contains("://") {
                return None;
            }
            let (host, path) = raw.split_once(':')?;
            if host.contains('/') || host.contains('\\') {
                return None;
            }
            (
                host.rsplit('@').next()?.to_string(),
                path.trim_start_matches('/').to_string(),
                false,
            )
        };
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return None;
    }
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let parts: Vec<_> = path.split('/').collect();
    if parts.is_empty()
        || parts.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c))
        })
    {
        return None;
    }
    let provider = match host.as_str() {
        "github.com" => Some("GitHub"),
        "gitlab.com" => Some("GitLab"),
        "bitbucket.org" => Some("Bitbucket"),
        _ => None,
    }
    .map(str::to_string);
    let web_url = if http || provider.is_some() {
        Some(format!("https://{host}/{path}"))
    } else {
        None
    };
    Some(RepositoryInfo {
        provider,
        owner: (parts.len() > 1).then(|| parts[..parts.len() - 1].join("/")),
        repository: parts.last()?.to_string(),
        remote_url: format!("{host}/{path}"),
        web_url,
    })
}
