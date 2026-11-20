//! Application identity and defaults for bounded HTTP requests.

use std::time::Duration;
use ureq::{Agent, config::Config};

pub(crate) const USER_AGENT: &str = concat!("Riina/", env!("CARGO_PKG_VERSION"));

pub(crate) fn agent(timeout: Duration) -> Agent {
    agent_config(timeout, RequestPolicy::Default).new_agent()
}

pub(crate) enum RequestPolicy {
    Default,
    HttpsWithoutRedirects,
    ManualRedirects,
}

pub(crate) fn agent_config(timeout: Duration, policy: RequestPolicy) -> Config {
    let builder = Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(USER_AGENT);
    match policy {
        RequestPolicy::Default => builder,
        RequestPolicy::HttpsWithoutRedirects => builder.https_only(true).max_redirects(0),
        // Plugin JavaScript receives the HTTP status even for error responses.
        RequestPolicy::ManualRedirects => builder.max_redirects(0).http_status_as_error(false),
    }
    .build()
}

pub(crate) fn format_api_error(
    provider: &str,
    action: &str,
    error: ureq::Error,
    overrides: &[(u16, &str)],
) -> String {
    match error {
        ureq::Error::StatusCode(status) => overrides
            .iter()
            .find_map(|&(code, message)| (code == status).then_some(message))
            .map_or_else(
                || format!("{provider} {action} failed with HTTP {status}."),
                str::to_owned,
            ),
        error => format!("{provider} {action} failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_messages_override_only_their_status_codes() {
        let overrides = [(429, "Try again later.")];
        assert_eq!(
            format_api_error(
                "Provider",
                "search",
                ureq::Error::StatusCode(429),
                &overrides
            ),
            "Try again later."
        );
        assert_eq!(
            format_api_error(
                "Provider",
                "search",
                ureq::Error::StatusCode(503),
                &overrides
            ),
            "Provider search failed with HTTP 503."
        );
        let error = ureq::Error::HostNotFound;
        let expected = format!("Provider search failed: {error}");
        assert_eq!(
            format_api_error("Provider", "search", error, &overrides),
            expected
        );
    }
}
