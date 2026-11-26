use super::DecodeError;
use objc2_core_foundation::CFString;
pub(super) use objc2_core_foundation::CFStringEncodings as Encoding;

pub(super) fn decode(bytes: &[u8], encoding: Encoding) -> Result<String, DecodeError> {
    let encoding = encoding.0 as u32;
    if !CFString::is_encoding_available(encoding) {
        return Err(DecodeError::Unsupported);
    }
    // SAFETY: the bytes remain live for the call and None selects the default allocator.
    unsafe { CFString::with_bytes(None, bytes.as_ptr(), bytes.len() as _, encoding, true) }
        .map(|value| value.to_string())
        .ok_or(DecodeError::Invalid)
}
