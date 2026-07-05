use std::env;

use super::ScrapeError;

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn allowlist_entry_host(allowed_entry: &str) -> &str {
    if let Some(rest) = allowed_entry.strip_prefix('[') {
        if let Some((host, _)) = rest.split_once(']') {
            return host;
        }
    }

    if let Some((host, port)) = allowed_entry.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()) {
            return host;
        }
    }

    allowed_entry
}

fn allowlist_entry_matches_host(allowed_entry: &str, host: &str, host_with_port: &str) -> bool {
    if allowed_entry == host_with_port || allowed_entry == host {
        return true;
    }

    let allowed_host = allowlist_entry_host(allowed_entry);

    is_loopback_host(host) && is_loopback_host(allowed_host)
}

/// Check if a URL's host is allowed for scraping.
/// If SCRAPE_ALLOWED_HOSTS is set, only those hosts are allowed.
/// If not set, all hosts are allowed (production mode).
pub fn is_host_allowed(url: &str) -> Result<(), ScrapeError> {
    let parsed = reqwest::Url::parse(url).map_err(|e| ScrapeError::InvalidUrl(e.to_string()))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| ScrapeError::InvalidUrl("No host in URL".to_string()))?;

    // Check for allowed hosts (used in tests)
    if let Ok(allowed) = env::var("SCRAPE_ALLOWED_HOSTS") {
        let allowed_hosts: Vec<&str> = allowed.split(',').map(|s| s.trim()).collect();
        // Include port if present
        let host_with_port = if let Some(port) = parsed.port() {
            format!("{}:{}", host, port)
        } else {
            host.to_string()
        };

        if !allowed_hosts
            .iter()
            .any(|&h| allowlist_entry_matches_host(h, host, &host_with_port))
        {
            return Err(ScrapeError::HostNotAllowed(host_with_port));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist_allows_loopback_on_other_ports() {
        assert!(allowlist_entry_matches_host(
            "localhost:62872",
            "localhost",
            "localhost:57565"
        ));
    }

    #[test]
    fn host_allowlist_still_rejects_other_hosts() {
        assert!(!allowlist_entry_matches_host(
            "localhost:62872",
            "www.seriouseats.com",
            "www.seriouseats.com"
        ));
    }

    #[test]
    fn host_allowlist_allows_ipv6_loopback_on_other_ports() {
        assert!(allowlist_entry_matches_host(
            "::1:62872",
            "::1",
            "::1:57565"
        ));
    }
}
