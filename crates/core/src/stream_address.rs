use crate::media_reference::{NetworkUrlError, validate_network_url};
use std::borrow::Cow;

pub struct StreamAddress<'a> {
    pub url: Cow<'a, str>,
    pub uses_default_scheme: bool,
}

/// Interpret text entered into the stream dialog. Startup arguments and
/// playlist data continue to require a complete, explicit network URL.
pub fn stream_address(input: &str) -> Result<StreamAddress<'_>, NetworkUrlError> {
    let input = input.trim();
    let drive_path = matches!(input.as_bytes(), [letter, b':', b'/' | b'\\', ..] if letter.is_ascii_alphabetic());
    if input.starts_with(['/', '\\']) || input.is_empty() || drive_path {
        return Err(NetworkUrlError::MissingHost);
    }

    let explicit_scheme = input
        .split_once("://")
        .is_some_and(|(prefix, _)| !prefix.contains(['/', '?', '#']));
    let (url, uses_default_scheme) = if explicit_scheme {
        (Cow::Borrowed(input), false)
    } else {
        (Cow::Owned(format!("http://{input}")), true)
    };
    let (scheme, rest) = url.split_once("://").ok_or(NetworkUrlError::MissingHost)?;
    let (authority, resource) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    if authority.is_empty() || authority.contains('\\') {
        return Err(NetworkUrlError::MissingHost);
    }
    if authority.chars().any(char::is_whitespace) || url.chars().any(char::is_control) {
        return Err(NetworkUrlError::InvalidCharacters);
    }

    let encoded_authority = encode_authority(authority)?;
    let encoded_resource = encode_url_text(resource);
    let url = if encoded_authority == authority && encoded_resource == resource {
        url
    } else {
        Cow::Owned(format!("{scheme}://{encoded_authority}{encoded_resource}"))
    };
    validate_network_url(&url)?;
    Ok(StreamAddress {
        url,
        uses_default_scheme,
    })
}

fn encode_authority(authority: &str) -> Result<Cow<'_, str>, NetworkUrlError> {
    if authority.is_ascii() {
        return Ok(Cow::Borrowed(authority));
    }
    let (account, endpoint) = authority
        .rsplit_once('@')
        .map_or((None, authority), |(account, endpoint)| {
            (Some(account), endpoint)
        });
    let mut encoded = String::new();
    if let Some(account) = account {
        encoded.push_str(&encode_url_text(account));
        encoded.push('@');
    }
    if endpoint.is_ascii() {
        encoded.push_str(endpoint);
    } else {
        let (host, port) = endpoint.split_at(endpoint.find(':').unwrap_or(endpoint.len()));
        let host = url::Host::parse(host).map_err(NetworkUrlError::Parse)?;
        encoded.push_str(&host.to_string());
        encoded.push_str(port);
    }
    Ok(Cow::Owned(encoded))
}

fn encode_url_text(value: &str) -> Cow<'_, str> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = value.as_bytes();
    let needs_escape = |index, byte: u8| {
        if byte == b'%' {
            !bytes
                .get(index + 1..index + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
        } else {
            !byte.is_ascii_alphanumeric() && !b"-._~:/?#[]@!$&'()*+,;=".contains(&byte)
        }
    };
    let Some(first) = bytes
        .iter()
        .copied()
        .enumerate()
        .position(|(index, byte)| needs_escape(index, byte))
    else {
        return Cow::Borrowed(value);
    };
    // Keep already escaped bytes and their spelling, including query tokens.
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push_str(&value[..first]);
    for (index, byte) in bytes.iter().copied().enumerate().skip(first) {
        if needs_escape(index, byte) {
            escaped.push('%');
            escaped.push(char::from(HEX[usize::from(byte >> 4)]));
            escaped.push(char::from(HEX[usize::from(byte & 0x0f)]));
        } else {
            escaped.push(char::from(byte));
        }
    }
    Cow::Owned(escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_accepts_a_host_and_port_and_preserves_explicit_escaping() {
        for (input, expected, default_scheme) in [
            (
                " example.test:1234/live ",
                "http://example.test:1234/live",
                true,
            ),
            (
                "[::1]:8000/movie name",
                "http://[::1]:8000/movie%20name",
                true,
            ),
            (
                "HTTPS://example.test/clip?value=a%2Fb",
                "HTTPS://example.test/clip?value=a%2Fb",
                false,
            ),
            (
                "ytdl://https://example.test/watch?q=50%25",
                "ytdl://https://example.test/watch?q=50%25",
                false,
            ),
            (
                "münchen.example:8080/Résumé 片.avi?sig=%2f+Z&caption=日本語#part 1",
                concat!(
                    "http://xn--mnchen-3ya.example:8080/R%C3%A9sum%C3%A9%20%E7%89%87.avi",
                    "?sig=%2f+Z&caption=%E6%97%A5%E6%9C%AC%E8%AA%9E#part%201"
                ),
                true,
            ),
            (
                "rtsp://münchen.example:8554/show {50%}.mkv",
                "rtsp://xn--mnchen-3ya.example:8554/show%20%7B50%25%7D.mkv",
                false,
            ),
            (
                "https://usér:clé@[::1]:8443/二 番.avi?escaped=%20",
                "https://us%C3%A9r:cl%C3%A9@[::1]:8443/%E4%BA%8C%C2%A0%E7%95%AA.avi?escaped=%20",
                false,
            ),
        ] {
            let address = stream_address(input).unwrap();
            assert_eq!(address.url, expected);
            assert_eq!(address.uses_default_scheme, default_scheme);
        }
    }

    #[test]
    fn invalid_and_local_references_are_never_sent_to_the_engine_as_streams() {
        for input in [
            "",
            "/tmp/movie.mkv",
            "\\server\\share",
            "C:\\Videos\\movie.mkv",
            "C:/Videos/movie.mkv",
            "file:///tmp/movie.mkv",
            "https:///tmp/movie.mkv",
            "https://bad host/stream",
            "example.test/live\nstream",
            "javascript://example.test/",
        ] {
            assert!(stream_address(input).is_err(), "{input:?}");
        }
    }
}
