use super::DecodeError;
use std::ffi::CStr;

pub(super) fn decode(bytes: &[u8], encoding: Option<&CStr>) -> Result<String, DecodeError> {
    let encoding = encoding.ok_or(DecodeError::Unsupported)?;
    // SAFETY: both encoding names are live, terminated C strings.
    let descriptor = unsafe { libc::iconv_open(c"UTF-8".as_ptr(), encoding.as_ptr()) };
    if descriptor as usize == usize::MAX {
        return Err(DecodeError::Unsupported);
    }
    let mut output = vec![0_u8; bytes.len() * 4 + 4];
    let mut from = bytes.as_ptr().cast_mut().cast();
    let mut remaining = bytes.len();
    let mut to = output.as_mut_ptr().cast();
    let mut capacity = output.len();
    // SAFETY: iconv reads from the input, writes within the declared output capacity,
    // and advances the local pointers. This call owns the descriptor until close.
    let converted = unsafe {
        let result = libc::iconv(
            descriptor,
            &mut from,
            &mut remaining,
            &mut to,
            &mut capacity,
        );
        libc::iconv_close(descriptor);
        result
    };
    if converted != 0 || remaining != 0 {
        return Err(DecodeError::Invalid);
    }
    output.truncate(output.len() - capacity);
    String::from_utf8(output).map_err(|_| DecodeError::Invalid)
}
