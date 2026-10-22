//! Strict decoding for the plugin file API encoding names.
pub(super) fn decode_text(bytes: &[u8], name: &str) -> Result<String, String> {
    let label = name.trim().to_ascii_lowercase().replace('_', "-");
    let label = match label.as_str() {
        "utf8" => "utf-8",
        "utf16" | "unicode" => "utf-16",
        "utf16littleendian" => "utf-16le",
        "utf16bigendian" => "utf-16be",
        "utf32" => "utf-32",
        "utf32littleendian" => "utf-32le",
        "utf32bigendian" => "utf-32be",
        "japaneseuc" => "euc-jp",
        "shiftjis" => "shift_jis",
        "macosroman" => "macintosh",
        "big5-hkscs" | "big5-hkscs-1999" | "big5-e" => "big5",
        "iso-2022-jp-1" | "iso-2022-jp-2" | "iso-2022-jp-3" => "iso-2022-jp",
        "windowscp1250" => "windows-1250",
        "windowscp1251" => "windows-1251",
        "windowscp1252" => "windows-1252",
        "windowscp1253" => "windows-1253",
        "windowscp1254" => "windows-1254",
        "windowsarabic" => "windows-1256",
        "windowsbalticrim" => "windows-1257",
        "windowscyrillic" => "windows-1251",
        "windowsgreek" => "windows-1253",
        "windowshebrew" => "windows-1255",
        "windowslatin2" => "windows-1250",
        "windowslatin5" => "windows-1254",
        "windowsvietnamese" => "windows-1258",
        "isolatin5" => "iso-8859-9",
        "isolatinthai" => "windows-874",
        "cp949" => "euc-kr",
        "iso2022jp" | "iso-2022-jp" => "iso-2022-jp",
        "euc-cn" | "gbk-95" | "gb2312-80" => "gbk",
        "gb-18030-2000" => "gb18030",
        "latin1" | "isolatin1" | "iso-8859-1" => "x-riina-latin1",
        "latin2" | "isolatin2" => "iso-8859-2",
        "latin3" | "isolatin3" => "iso-8859-3",
        "latin4" | "isolatin4" => "iso-8859-4",
        "latin6" | "isolatin6" => "iso-8859-10",
        "latin7" | "isolatin7" => "iso-8859-13",
        "latin8" | "isolatin8" => "iso-8859-14",
        "latin9" | "isolatin9" => "iso-8859-15",
        "latin10" | "isolatin10" => "iso-8859-16",
        "isolatinarabic" => "iso-8859-6",
        "isolatincyrillic" => "iso-8859-5",
        "isolatingreek" => "iso-8859-7",
        "isolatinhebrew" => "iso-8859-8",
        other => other,
    };

    if label == "x-riina-latin1" {
        return Ok(bytes.iter().map(|byte| char::from(*byte)).collect());
    }
    if label == "ascii" {
        return bytes
            .iter()
            .copied()
            .map(|byte| {
                if byte.is_ascii() {
                    Ok(char::from(byte))
                } else {
                    Err("Input contains bytes outside ASCII".to_owned())
                }
            })
            .collect();
    }
    if matches!(label, "utf-32" | "utf-32le" | "utf-32be") {
        return decode_utf32(bytes, label);
    }

    let requested = if label == "utf-16" {
        match bytes {
            [0xff, 0xfe, ..] => "utf-16le",
            [0xfe, 0xff, ..] => "utf-16be",
            _ => "utf-16le",
        }
    } else {
        label
    };
    let encoding = encoding_rs::Encoding::for_label_no_replacement(requested.as_bytes())
        .ok_or_else(|| format!("Unknown or unsupported text encoding: {name}"))?;
    let bytes = if (encoding == encoding_rs::UTF_8 && bytes.starts_with(&[0xef, 0xbb, 0xbf]))
        || (encoding == encoding_rs::UTF_16LE && bytes.starts_with(&[0xff, 0xfe]))
        || (encoding == encoding_rs::UTF_16BE && bytes.starts_with(&[0xfe, 0xff]))
    {
        &bytes[if encoding == encoding_rs::UTF_8 { 3 } else { 2 }..]
    } else {
        bytes
    };
    encoding
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
        .ok_or_else(|| format!("File contains invalid {name} text"))
}

fn decode_utf32(bytes: &[u8], label: &str) -> Result<String, String> {
    let (big_endian, bytes) = match label {
        "utf-32" if bytes.starts_with(&[0x00, 0x00, 0xfe, 0xff]) => (true, &bytes[4..]),
        "utf-32" if bytes.starts_with(&[0xff, 0xfe, 0x00, 0x00]) => (false, &bytes[4..]),
        "utf-32" | "utf-32le" => (
            false,
            bytes.strip_prefix(&[0xff, 0xfe, 0, 0]).unwrap_or(bytes),
        ),
        "utf-32be" => (
            true,
            bytes.strip_prefix(&[0, 0, 0xfe, 0xff]).unwrap_or(bytes),
        ),
        _ => unreachable!("UTF-32 labels are validated by the caller"),
    };
    if bytes.len() % 4 != 0 {
        return Err("UTF-32 input has an incomplete code point".into());
    }
    let mut text = String::with_capacity(bytes.len() / 2);
    for chunk in bytes.as_chunks::<4>().0 {
        let code_point = if big_endian {
            u32::from_be_bytes(*chunk)
        } else {
            u32::from_le_bytes(*chunk)
        };
        let character = char::from_u32(code_point)
            .ok_or_else(|| "UTF-32 input contains an invalid Unicode scalar".to_owned())?;
        text.push(character);
    }
    Ok(text)
}
