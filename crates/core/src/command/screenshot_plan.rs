use super::set;
use crate::{screenshot, valid_path};

pub(super) fn plan(
    directory: &str,
    content: screenshot::Content,
    encoding: &screenshot::Encoding,
) -> Result<Vec<Vec<String>>, String> {
    if !valid_path(directory) || encoding.template.contains('\0') {
        return Err("Screenshot directory is empty or settings contain a NUL byte".into());
    }
    Ok(vec![
        set("screenshot-format", encoding.format.native_value()),
        set("screenshot-template", encoding.template.clone()),
        // Match the reference's JXL software path without changing
        // screenshot conversion policy for the other formats.
        set(
            "screenshot-sw",
            if encoding.format == screenshot::Format::Jxl {
                "yes"
            } else {
                "no"
            },
        ),
        // Keep the native trailing separator: mixed '/' in a
        // verbatim Windows path is rejected by screenshot writing.
        set(
            "screenshot-dir",
            std::path::Path::new(directory)
                .join("")
                .to_string_lossy()
                .into_owned(),
        ),
        vec!["screenshot".into(), content.native_value().into()],
    ])
}
