//! Read only the destination from an explicitly selected internet shortcut.
use riina_core::stream_address::explicit_stream_url;
use std::{borrow::Cow, fs, io::Read, path::Path};

const MAX_BYTES: u64 = 16 * 1024;

pub(super) fn is_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("url"))
}

pub(super) fn resolve(path: String) -> Result<String, String> {
    if path.contains('\0') || !is_path(&path) {
        return Ok(path);
    }
    let metadata = fs::metadata(&path).map_err(read_error)?;
    if metadata.is_dir() {
        return Ok(path);
    }
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("An internet shortcut must be a small regular file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(&path)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(read_error)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The internet shortcut is too large".into());
    }
    let text = decode(&bytes).ok_or("The internet shortcut contains invalid text")?;
    let url = destination(&text).ok_or("The internet shortcut has no supported stream URL")?;
    Ok(url.into_owned())
}

fn read_error(error: std::io::Error) -> String {
    format!("Could not read the internet shortcut: {error}")
}

fn decode(bytes: &[u8]) -> Option<Cow<'_, str>> {
    let (remainder, decode_unit): (_, fn([u8; 2]) -> u16) = match bytes {
        [0xff, 0xfe, rest @ ..] => (rest, u16::from_le_bytes),
        [0xfe, 0xff, rest @ ..] => (rest, u16::from_be_bytes),
        _ => {
            let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
            return std::str::from_utf8(bytes).ok().map(Cow::Borrowed);
        }
    };
    let (units, tail) = remainder.as_chunks::<2>();
    if !tail.is_empty() {
        return None;
    }
    let units = units.iter().copied().map(decode_unit).collect::<Vec<_>>();
    String::from_utf16(&units).ok().map(Cow::Owned)
}

fn destination(contents: &str) -> Option<Cow<'_, str>> {
    if contents.contains('\0') {
        return None;
    }
    let mut in_internet_shortcut = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_internet_shortcut = line.eq_ignore_ascii_case("[InternetShortcut]");
        } else if in_internet_shortcut
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("URL")
        {
            return explicit_stream_url(value.trim());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internet_shortcuts_resolve_only_validated_network_urls() {
        let bom_text = b"\xef\xbb\xbf[Other]\nURL=https://wrong.test/\r\n\
                         [InternetShortcut]\r\nURL=RTSP://camera.test/live\r\n";
        let text = decode(bom_text).unwrap();
        assert_eq!(
            destination(&text).as_deref(),
            Some("RTSP://camera.test/live")
        );
        for contents in [
            "URL=https://outside-section.test/\n",
            "[InternetShortcut]\nURL=file:///secret\n",
            "[InternetShortcut]\nURL=https://bad host/\n",
            "[InternetShortcut]\nURL=example.test/movie\n",
            "[InternetShortcut]\nIconFile=shortcut.ico\n",
            "[InternetShortcut]\nURL=https://example.test/a\0b\n",
        ] {
            assert!(destination(contents).is_none(), "accepted {contents:?}");
        }
        assert!(decode(b"\xff\xfe\x01").is_none());
        assert!(decode(b"\xfe\xff\xd8\x00").is_none());
    }

    #[test]
    fn shortcut_size_limits_leave_folders_available_to_discovery() {
        let scratch = riina_test_support::TempDir::new("internet-shortcut-bounds").unwrap();
        let folder = scratch.path().join("media.url");
        fs::create_dir(&folder).unwrap();
        let folder = folder.into_os_string().into_string().unwrap();
        assert_eq!(resolve(folder.clone()).unwrap(), folder);

        let oversized = scratch.path().join("oversized.url");
        fs::write(&oversized, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(resolve(oversized.into_os_string().into_string().unwrap()).is_err());
    }
}
