//! Native plugin commands that can use argv without opening new resources.
use super::CommandError;

pub const MAX_PLUGIN_MPV_ARGUMENTS: usize = 64;
pub const MAX_PLUGIN_MPV_ARGUMENT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMpvCommand {
    arguments: Vec<String>,
}

impl PluginMpvCommand {
    pub fn new(name: &str, mut arguments: Vec<String>) -> Result<Self, CommandError> {
        if !SUPPORTED.contains(&name) {
            return Err(CommandError::InvalidArgument {
                what: "This native command is outside the plugin playback capability",
            });
        }
        validate_plugin_mpv_arguments(&arguments)?;
        arguments.insert(0, name.into());
        Ok(Self { arguments })
    }

    pub fn name(&self) -> &str {
        &self.arguments[0]
    }

    pub(super) fn arguments(&self) -> Vec<String> {
        self.arguments.clone()
    }
}

pub fn validate_plugin_mpv_arguments(arguments: &[String]) -> Result<(), CommandError> {
    if arguments.len() > MAX_PLUGIN_MPV_ARGUMENTS
        || arguments.iter().any(|argument| argument.contains('\0'))
        || arguments
            .iter()
            .map(String::len)
            .fold(0usize, usize::saturating_add)
            > MAX_PLUGIN_MPV_ARGUMENT_BYTES
    {
        return Err(CommandError::InvalidArgument {
            what: "Plugin mpv arguments exceed 64 entries or 16 KiB, or contain NUL",
        });
    }
    Ok(())
}

// Public command names from the pinned playback capability inventory.
// Quit, rendering callbacks and resource loads have separate ownership.
const SUPPORTED: &[&str] = &[
    "ab-loop",
    "ab-loop-align-cache",
    "audio-reload",
    "audio-remove",
    "drop-buffers",
    "escape-ass",
    "expand-text",
    "frame-back-step",
    "frame-step",
    "ignore",
    "osd-overlay",
    "overlay-remove",
    "playlist-clear",
    "playlist-move",
    "playlist-next",
    "playlist-next-playlist",
    "playlist-play-index",
    "playlist-prev",
    "playlist-prev-playlist",
    "playlist-remove",
    "playlist-shuffle",
    "playlist-unshuffle",
    "print-text",
    "revert-seek",
    "screenshot-raw",
    "seek",
    "show-progress",
    "show-text",
    "stop",
    "sub-reload",
    "sub-remove",
    "sub-seek",
    "sub-step",
    "video-reload",
    "video-remove",
];
