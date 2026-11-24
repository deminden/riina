use std::{io::Read, path::Path};
use wl_clipboard_rs::{
    copy::{MimeSource, MimeType, Options, Source},
    paste,
};

const URI_LIST: &str = "text/uri-list";

pub(super) fn copy_file_list(paths: &[String], text: &str) -> Result<(), String> {
    let uris = encode_file_list(paths).ok_or("Could not encode the playlist as file URLs")?;
    Options::new()
        .copy_multi(vec![
            MimeSource {
                source: Source::Bytes(text.as_bytes().into()),
                mime_type: MimeType::Text,
            },
            MimeSource {
                source: Source::Bytes(uris.into_bytes().into_boxed_slice()),
                mime_type: MimeType::Specific(URI_LIST.to_owned()),
            },
        ])
        .map_err(|error| error.to_string())
}

fn encode_file_list(paths: &[String]) -> Option<String> {
    let mut list = String::new();
    for path in paths {
        let url = url::Url::from_file_path(Path::new(path)).ok()?;
        list.push_str(url.as_str());
        list.push_str("\r\n");
    }
    Some(list)
}

// None leaves transport selection to arboard, including its X11 fallback.
pub(super) fn read_references() -> Option<Result<Vec<String>, String>> {
    use paste::{ClipboardType::Regular, Error, MimeType, Seat::Unspecified};

    let contents = paste::get_contents(Regular, Unspecified, MimeType::TextWithPriority(URI_LIST));
    let (reader, mime) = match contents {
        Ok(contents) => contents,
        Err(
            Error::SocketOpenError(_) | Error::WaylandConnection(_) | Error::MissingProtocol { .. },
        ) => {
            return None;
        }
        Err(error) => return Some(Err(error.to_string())),
    };
    let references = read_text(reader).and_then(|text| {
        if mime != URI_LIST {
            return Ok(riina_core::playlist::clipboard_references(&text));
        }
        let paths = decode_uri_list(&text);
        if !paths.is_empty() {
            return Ok(paths);
        }
        let (reader, mime) = paste::get_contents(Regular, Unspecified, MimeType::Text)
            .map_err(|error| error.to_string())?;
        if mime == URI_LIST {
            return Ok(Vec::new());
        }
        read_text(reader).map(|text| riina_core::playlist::clipboard_references(&text))
    });
    Some(references)
}

fn read_text(mut reader: impl Read) -> Result<String, String> {
    let mut text = String::new();
    reader
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    Ok(text)
}

fn decode_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| {
            line.split_once(':')
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
                || riina_core::media_reference::validate_network_url(line).is_ok()
        })
        .flat_map(riina_core::playlist::clipboard_references)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{decode_uri_list, encode_file_list};

    #[test]
    fn uri_list_round_trip_preserves_special_filename_bytes_and_order() {
        let paths = vec!["/media/a %é#\r".into(), "/media/b\nclip.mp4".into()];
        let encoded = encode_file_list(&paths).unwrap();
        assert_eq!(
            encoded,
            "file:///media/a%20%25%C3%A9%23%0D\r\nfile:///media/b%0Aclip.mp4\r\n"
        );
        assert_eq!(decode_uri_list(&encoded), paths);
    }

    #[test]
    fn uri_list_ignores_comments_relative_paths_and_unsupported_file_authority() {
        assert_eq!(
            decode_uri_list(
                "# File manager\r\nfile://host/folder\r\nrelative.txt\n\
                 FILE://localhost/media/a%23b.mkv\r\nhttps://example.com/video.mp4\n",
            ),
            ["/media/a#b.mkv", "https://example.com/video.mp4"]
        );
    }
}
