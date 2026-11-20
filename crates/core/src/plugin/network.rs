//! Interpret manifest grants independently of the HTTP transport.
use std::net::IpAddr;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkGrant {
    Exact,
    Wildcard,
}

pub fn network_url_allowed(value: &str, domains: &[String]) -> bool {
    Url::parse(value).is_ok_and(|url| network_url_grant(&url, domains).is_some())
}

pub fn network_url_grant(url: &Url, domains: &[String]) -> Option<NetworkGrant> {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let address = literal_address(&host);
    if domains.iter().any(|pattern| {
        pattern.eq_ignore_ascii_case(&host)
            || (address.is_some() && address == literal_address(pattern))
    }) {
        return Some(NetworkGrant::Exact);
    }
    if network_host_is_restricted(&host) {
        return None;
    }
    domains
        .iter()
        .any(|pattern| {
            pattern == "*"
                || pattern.strip_prefix("*.").is_some_and(|suffix| {
                    host.strip_suffix(&suffix.to_ascii_lowercase())
                        .is_some_and(|prefix| prefix.len() > 1 && prefix.ends_with('.'))
                })
        })
        .then_some(NetworkGrant::Wildcard)
}

pub(super) fn literal_address(host: &str) -> Option<IpAddr> {
    let host = host.trim_end_matches('.');
    host.strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

pub fn network_host_is_restricted(host: &str) -> bool {
    let hostname = host.trim_end_matches('.').to_ascii_lowercase();
    hostname == "localhost"
        || hostname.ends_with(".localhost")
        || literal_address(&hostname).is_some_and(network_address_is_restricted)
}

pub fn network_address_is_restricted(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_loopback()
                || address.is_private()
                || address.is_link_local()
                || address.is_unspecified()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || address.is_unique_local()
                || address.is_unicast_link_local()
                || address.is_unspecified()
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| network_address_is_restricted(IpAddr::V4(mapped)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_private_hosts_and_normalizes_host_spelling() {
        for (host, restricted) in [
            ("LOCALHOST.", true),
            ("player.LOCALHOST", true),
            ("127.0.0.1.", true),
            ("10.0.0.4", true),
            ("172.18.0.1", true),
            ("192.168.10.4", true),
            ("169.254.1.1", true),
            ("0.0.0.0", true),
            ("[::]", true),
            ("[::1]", true),
            ("[FE80::1]", true),
            ("[FD22::2]", true),
            ("[::ffff:192.168.1.2]", true),
            ("[::ffff:127.0.0.1]", true),
            ("[2001:db8::1]", false),
            ("[::ffff:198.51.100.1]", false),
            ("172.32.0.1", false),
            ("media.example.", false),
        ] {
            assert_eq!(network_host_is_restricted(host), restricted, "{host}");
        }
    }

    #[test]
    fn exact_local_grants_take_precedence_over_wildcards() {
        fn grant(value: &str, domains: &[&str]) -> Option<NetworkGrant> {
            network_url_grant(
                &Url::parse(value).unwrap(),
                &domains
                    .iter()
                    .map(|domain| (*domain).into())
                    .collect::<Vec<_>>(),
            )
        }
        assert_eq!(grant("https://LOCALHOST./", &["*"]), None);
        assert_eq!(grant("https://[::1]/", &["*"]), None);
        assert_eq!(grant("https://one.localhost/", &["*.localhost"]), None);
        assert_eq!(
            grant("https://localhost/", &["*", "LOCALHOST"]),
            Some(NetworkGrant::Exact)
        );
        assert_eq!(
            grant("http://[::1]/", &["[0:0:0:0:0:0:0:1]"]),
            Some(NetworkGrant::Exact)
        );
        assert_eq!(
            grant("http://192.168.1.2/", &["192.168.1.2"]),
            Some(NetworkGrant::Exact)
        );
        assert_eq!(
            grant("https://cdn.example.test./", &["*.example.test"]),
            Some(NetworkGrant::Wildcard)
        );
        assert_eq!(grant("https://example.test/", &["*.example.test"]), None);
    }
}
