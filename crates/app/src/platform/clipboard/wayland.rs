use super::uri_list::{MIME, decode};
use std::io::Read;
use wl_clipboard_rs::{
    copy::{MimeSource, MimeType, Options, Source},
    paste,
};

pub(super) fn references_available() -> Option<bool> {
    use paste::{ClipboardType::Regular, Error, MimeType, Seat::Unspecified};

    match paste::get_mime_types(Regular, Unspecified) {
        Ok(types) => Some(
            paste::select_mime_type(
                types.into_iter().collect(),
                MimeType::TextWithPriority(MIME),
            )
            .is_some(),
        ),
        Err(
            Error::SocketOpenError(_) | Error::WaylandConnection(_) | Error::MissingProtocol { .. },
        ) => None,
        Err(_) => Some(false),
    }
}

pub(super) fn copy_file_list(uris: &str, text: &str) -> Result<(), String> {
    Options::new()
        .copy_multi(vec![
            MimeSource {
                source: Source::Bytes(text.as_bytes().into()),
                mime_type: MimeType::Text,
            },
            MimeSource {
                source: Source::Bytes(uris.as_bytes().into()),
                mime_type: MimeType::Specific(MIME.to_owned()),
            },
        ])
        .map_err(|error| error.to_string())
}

// None leaves transport selection to arboard, including its X11 fallback.
pub(super) fn read_references() -> Option<Result<Vec<String>, String>> {
    use paste::{ClipboardType::Regular, Error, MimeType, Seat::Unspecified};

    let contents = paste::get_contents(Regular, Unspecified, MimeType::TextWithPriority(MIME));
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
        if mime != MIME {
            return Ok(riina_core::playlist::clipboard_references(&text));
        }
        let paths = decode(&text);
        if !paths.is_empty() {
            return Ok(paths);
        }
        let (reader, mime) = paste::get_contents(Regular, Unspecified, MimeType::Text)
            .map_err(|error| error.to_string())?;
        if mime == MIME {
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
