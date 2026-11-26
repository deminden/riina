use super::{Command, CommandError, set};
use crate::{screenshot, valid_path};

pub(super) fn plan(command: &Command) -> Result<Vec<Vec<String>>, CommandError> {
    let (directory, encoding, capture) = match command {
        Command::Screenshot {
            directory,
            content,
            encoding,
        } => (
            directory,
            encoding,
            vec!["screenshot".into(), content.native_value().into()],
        ),
        Command::ScreenshotBinding {
            directory,
            default_content,
            encoding,
            binding,
        } => (
            directory,
            encoding,
            binding.clone().native_arguments(*default_content),
        ),
        _ => {
            return Err(CommandError::InvalidArgument {
                what: "Expected a screenshot command",
            });
        }
    };
    if !valid_path(directory) || encoding.template.contains('\0') {
        return Err(CommandError::InvalidArgument {
            what: "Screenshot directory is empty or settings contain a NUL byte",
        });
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
        capture,
    ])
}
