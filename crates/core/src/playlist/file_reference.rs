//! Clipboard file URLs retain their authority; a network share is never a relative local path.

#[derive(Clone, Copy)]
enum PathConvention {
    Windows,
    Posix,
}

pub(super) fn clipboard_path(reference: &str) -> Option<String> {
    path(
        reference,
        if cfg!(windows) {
            PathConvention::Windows
        } else {
            PathConvention::Posix
        },
    )
}

fn path(reference: &str, convention: PathConvention) -> Option<String> {
    let (scheme, location) = reference.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") || location.contains(['?', '#', '\\']) {
        return None;
    }
    let (authority, encoded_path) = if let Some(location) = location.strip_prefix("//") {
        let boundary = location.find('/')?;
        (&location[..boundary], &location[boundary..])
    } else {
        ("", location)
    };
    if !encoded_path.starts_with('/') {
        return None;
    }
    let decoded = decode(encoded_path)?;
    if authority.is_empty() || authority.eq_ignore_ascii_case("localhost") {
        if matches!(convention, PathConvention::Windows) {
            let bytes = decoded.as_bytes();
            if bytes.get(1).is_some_and(u8::is_ascii_alphabetic) && bytes.get(2) == Some(&b':') {
                return Some(decoded[1..].to_owned());
            }
        }
        return Some(decoded);
    }
    // RFC 8089 treats localhost as local. Other authorities need a native
    // mechanism: Windows UNC preserves the server/share, while POSIX paths do not.
    if matches!(convention, PathConvention::Posix)
        || authority
            .chars()
            .any(|c| !c.is_alphanumeric() && !".-_".contains(c))
        || decoded.trim_start_matches('/').is_empty()
    {
        return None;
    }
    Some(format!("\\\\{authority}{}", decoded.replace('/', "\\")))
}

fn decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let result = String::from_utf8(decoded).ok()?;
    (!result.contains('\0')).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::{
        PathConvention::{Posix, Windows},
        path,
    };

    #[test]
    fn local_urls_preserve_unicode_spaces_and_native_drive_paths() {
        assert_eq!(
            path("FILE://localhost/C:/media/%E5%AD%97%20clip.mkv", Windows).as_deref(),
            Some("C:/media/字 clip.mkv")
        );
        assert_eq!(
            path("file:/home/media/a%23b.mkv", Posix).as_deref(),
            Some("/home/media/a#b.mkv")
        );
    }

    #[test]
    fn remote_authority_is_preserved_only_where_unc_paths_are_supported() {
        assert_eq!(
            path("file://server/share/my%20video.mkv", Windows).as_deref(),
            Some("\\\\server\\share\\my video.mkv")
        );
        assert!(path("file://server/share/video.mkv", Posix).is_none());
    }

    #[test]
    fn invalid_file_references_do_not_become_playlist_paths() {
        for value in [
            "file:relative.mkv",
            "file://server",
            "file://user@server/share/a",
            "file:///clip%00.mkv",
            "file:///clip%ZZ.mkv",
            "file:///clip%FF.mkv",
            "file:///clip.mkv#fragment",
        ] {
            assert!(path(value, Windows).is_none(), "accepted {value}");
        }
    }
}
