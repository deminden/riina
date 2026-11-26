use super::DecodeError;
use ::windows::Win32::Globalization::{IsValidCodePage, MB_ERR_INVALID_CHARS, MultiByteToWideChar};

pub(super) fn decode(bytes: &[u8], codepage: Option<u32>) -> Result<String, DecodeError> {
    let codepage = codepage.ok_or(DecodeError::Unsupported)?;
    // SAFETY: this query takes a numeric identifier and doesn't borrow any memory.
    if !unsafe { IsValidCodePage(codepage) }.as_bool() {
        return Err(DecodeError::Unsupported);
    }
    if bytes.is_empty() {
        return Ok(String::new());
    }
    // SAFETY: the binding supplies the input length and requests the required output size.
    let length = unsafe { MultiByteToWideChar(codepage, MB_ERR_INVALID_CHARS, bytes, None) };
    if length == 0 {
        return Err(DecodeError::Invalid);
    }
    let mut output = vec![0; length as usize];
    // SAFETY: the binding passes the length of both live buffers.
    let written =
        unsafe { MultiByteToWideChar(codepage, MB_ERR_INVALID_CHARS, bytes, Some(&mut output)) };
    if written != length {
        return Err(DecodeError::Invalid);
    }
    String::from_utf16(&output).map_err(|_| DecodeError::Invalid)
}
