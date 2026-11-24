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
    let authority_and_path = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or(NetworkUrlError::MissingHost)?;
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() || authority.contains('\\') {
        return Err(NetworkUrlError::MissingHost);
    }
    if authority.chars().any(char::is_whitespace) || url.chars().any(char::is_control) {
        return Err(NetworkUrlError::InvalidCharacters);
    }

    let url = if url.chars().any(char::is_whitespace) {
        Cow::Owned(url.replace(' ', "%20"))
    } else {
        url
    };
    validate_network_url(&url)?;
    Ok(StreamAddress {
        url,
        uses_default_scheme,
    })
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
