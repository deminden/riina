//! Translate plugin argv to shared player intents and reviewed native commands.
use super::{AvailablePermissions, core_api::plugin_media_reference};
use crate::plugin_files::PluginFiles;
use riina_core::{
    Command, PluginMpvCommand, control_preferences::SeekType, validate_plugin_mpv_arguments,
};

pub(super) enum Action {
    Playback(Command),
    SetProperty { name: String, value: String },
    Quit,
}

pub(super) fn plan(
    name: &str,
    args: Vec<String>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: &[String],
) -> Result<Action, String> {
    validate_plugin_mpv_arguments(&args).map_err(|error| error.to_string())?;
    if let Some(intent) = typed_playback(name, &args) {
        return Ok(Action::Playback(intent));
    }
    match name {
        "quit" if args.is_empty() || args == ["0"] => Ok(Action::Quit),
        "loadfile" | "sub-add" | "audio-add" | "video-add" => {
            require_file_access(name, permissions)?;
            let (path, options) = args.split_first().ok_or_else(|| invalid(name))?;
            let path = scoped_media_reference(path, files, permissions, allowed_domains)?;
            resource_intent(name, path, options).map(Action::Playback)
        }
        "set" => {
            require_file_access(name, permissions)?;
            let [property, value] = args.try_into().map_err(|_| invalid(name))?;
            Ok(Action::SetProperty {
                name: property,
                value,
            })
        }
        _ => PluginMpvCommand::new(name, args)
            .map(|native| Action::Playback(Command::PluginMpv(native)))
            .map_err(|error| error.to_string()),
    }
}

pub(super) fn scoped_media_reference(
    path: &str,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: &[String],
) -> Result<String, String> {
    if riina_core::media_reference::is_network_url_candidate(path)
        || url::Url::parse(path).is_ok_and(|url| url.scheme() == "file")
    {
        plugin_media_reference(
            path,
            files,
            permissions.file_system,
            permissions.network_request,
            allowed_domains,
        )
    } else {
        files
            .existing_path(path, permissions.file_system)
            .map(|path| path.to_string_lossy().into_owned())
    }
}

fn typed_playback(name: &str, args: &[String]) -> Option<Command> {
    match (name, args) {
        ("seek", [seconds]) => relative_seek(seconds, SeekType::Keyframe),
        ("seek", [seconds, flags]) => match flags.as_str() {
            "relative" | "relative+keyframes" => relative_seek(seconds, SeekType::Keyframe),
            "relative+exact" => relative_seek(seconds, SeekType::Exact),
            "absolute+exact" => Some(Command::Seek {
                seconds: finite(seconds)?,
                absolute: true,
            }),
            _ => None,
        },
        ("sub-seek", [direction]) => Some(Command::SubtitleSeek(direction.parse().ok()?)),
        ("stop", []) => Some(Command::Stop),
        ("frame-step", []) => Some(Command::FrameStep),
        ("frame-back-step", []) => Some(Command::FrameBackStep),
        ("ab-loop", []) => Some(Command::CycleAbLoop),
        ("playlist-next", []) => Some(Command::PlaylistNext),
        ("playlist-next", [strength]) if strength == "weak" => Some(Command::PlaylistNext),
        ("playlist-prev", []) => Some(Command::PlaylistPrevious),
        ("playlist-prev", [strength]) if strength == "weak" => Some(Command::PlaylistPrevious),
        ("playlist-clear", []) => Some(Command::ClearPlaylist),
        ("playlist-play-index", [index]) => Some(Command::SelectPlaylistEntry(index.parse().ok()?)),
        ("playlist-remove", [index]) => Some(Command::RemovePlaylistEntry(index.parse().ok()?)),
        ("playlist-move", [from, to]) => Some(Command::MovePlaylistEntry {
            from: from.parse().ok()?,
            to: to.parse().ok()?,
        }),
        _ => None,
    }
}

fn finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn relative_seek(seconds: &str, seek_type: SeekType) -> Option<Command> {
    Some(Command::SeekWithType {
        seconds: finite(seconds)?,
        seek_type,
    })
}

fn resource_intent(name: &str, path: String, options: &[String]) -> Result<Command, String> {
    match (name, options) {
        ("loadfile", []) => Ok(Command::Load(path)),
        ("loadfile", [mode]) if mode == "replace" => Ok(Command::Load(path)),
        ("loadfile", [mode]) if mode == "append" => Ok(Command::AppendFiles(vec![path])),
        ("sub-add", []) => Ok(Command::AddSubtitle(path)),
        ("audio-add", []) => Ok(Command::AddAudio(path)),
        ("video-add", []) => Ok(Command::AddVideo(path)),
        _ => Err(invalid(name)),
    }
}

fn require_file_access(name: &str, permissions: AvailablePermissions) -> Result<(), String> {
    if !permissions.file_system {
        return Err(format!(
            "mpv.command: {name} requires the file-system permission"
        ));
    }
    Ok(())
}

fn invalid(name: &str) -> String {
    format!("mpv.command: unexpected arguments for {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::Playback;

    #[test]
    fn plugin_commands_keep_literal_arguments_and_review_resource_access() {
        let files = PluginFiles::temporary("org.example.mpv-command-policy");
        let empty = AvailablePermissions::default();
        let playback = plan(
            "show-text",
            vec!["literal; stop".into()],
            &files,
            empty,
            &[],
        );
        let Ok(Action::Playback(command)) = playback else {
            panic!("Expected a native text command");
        };
        assert_eq!(
            command.plan(&Playback::default()).unwrap().commands,
            [["show-text", "literal; stop"]]
        );
        assert!(matches!(
            plan("playlist-next", vec![], &files, empty, &[]),
            Ok(Action::Playback(Command::PlaylistNext))
        ));
        assert!(matches!(
            plan("quit", vec![], &files, empty, &[]),
            Ok(Action::Quit)
        ));
        assert!(plan("quit", vec!["5".into()], &files, empty, &[]).is_err());
        assert!(plan("loadfile", vec!["example.mp4".into()], &files, empty, &[]).is_err());
        let granted = AvailablePermissions {
            file_system: true,
            ..empty
        };
        assert!(plan("run", vec!["example".into()], &files, granted, &[]).is_err());
        assert!(
            plan(
                "loadfile",
                vec!["https://example.test/media.mp4".into()],
                &files,
                granted,
                &[]
            )
            .is_err()
        );
        assert!(plan("show-text", vec!["bad\0text".into()], &files, empty, &[]).is_err());
    }
}
