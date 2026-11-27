use super::{Command, CommandError, LoadResume, MediaLink, cmd, flag, set};
use crate::{LoopMode, Phase, Playback, hdr_preferences};
use crate::{playlist_end_policy, valid_path};

impl Command {
    pub(super) fn playback_args(&self) -> Result<Vec<String>, CommandError> {
        let args = match self {
            Self::Load(path) if !path.is_empty() && !path.contains('\0') => {
                vec!["loadfile".into(), path.clone(), "replace".into()]
            }
            Self::Load(_) => {
                return Err(CommandError::InvalidArgument {
                    what: "The media path is empty or contains a NUL byte",
                });
            }
            Self::LoadAtPosition { path, position }
                if valid_path(path) && position.is_finite() && *position >= 0.0 =>
            {
                vec![
                    "loadfile".into(),
                    path.clone(),
                    "replace".into(),
                    "-1".into(),
                    format!("start={position}"),
                ]
            }
            Self::LoadAtPosition { .. } => {
                return Err(CommandError::InvalidArgument {
                    what: "The resume file or position is invalid",
                });
            }
            Self::SetPaused(paused) => set("pause", flag(*paused)),
            Self::TogglePause => cmd(["cycle", "pause"]),
            Self::SetPlaybackEndPolicy {
                keep_open_on_file_end,
                playlist_auto_play_next,
            } => set(
                "keep-open",
                playlist_end_policy(*keep_open_on_file_end, *playlist_auto_play_next),
            ),
            Self::Stop(_) => cmd(["stop"]),
            // The reference's custom field has no upper bound. Submit finite
            // requests above the engine limit so native rejection remains an
            // observed failure instead of inventing a narrower input contract.
            Self::Speed(value) if value.is_finite() && *value >= 0.01 => {
                set("speed", value.to_string())
            }
            Self::MultiplySpeed(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "speed".into(), factor.to_string()]
            }
            Self::FrameStep => cmd(["frame-step"]),
            Self::FrameBackStep => cmd(["frame-back-step"]),
            Self::CycleAbLoop => cmd(["ab-loop"]),
            Self::ToggleFileLoop => {
                // This shortcut changes only file repeat; a playlist repeat
                // setting survives while file repeat temporarily takes priority.
                cmd(["cycle-values", "loop-file", "inf", "no"])
            }
            _ => return Err(CommandError::OUT_OF_RANGE),
        };
        Ok(args)
    }
}

pub(super) fn stop(state: &Playback) -> Vec<Vec<String>> {
    if state.idle_for_finalization() || state.stopped_idle_by_engine() {
        // mpv can retain an idle playlist when stop has no current file.
        return vec![cmd(["playlist-clear"])];
    }
    let active = matches!(
        state.phase,
        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
    );
    let mut commands = Vec::with_capacity(2);
    if active && state.save_position_on_quit == Some(true) {
        commands.push(cmd(["write-watch-later-config"]));
    }
    commands.push(cmd(["stop"]));
    commands
}

pub(super) fn load(intent: &Command) -> Result<Vec<Vec<String>>, CommandError> {
    let mut commands = hdr_preferences::output_commands(false, None)?;
    commands.extend([set("pause", "yes"), intent.mpv_args()?]);
    Ok(commands)
}

pub(super) fn resume_on_open(command: &Command) -> Option<LoadResume> {
    match command {
        Command::Load(_)
        | Command::LoadAtPosition { .. }
        | Command::OpenFiles(_)
        | Command::OpenPlaylist(_)
        | Command::OpenShuffledPlaylist(_)
        | Command::OpenFilesWithSubtitles { .. } => Some(LoadResume::FollowPreference),
        Command::OpenMediaLink(MediaLink {
            append: false,
            pause,
            ..
        }) => match pause {
            None => Some(LoadResume::FollowPreference),
            Some(false) => Some(LoadResume::Play),
            Some(true) => None,
        },
        _ => None,
    }
}

pub(super) fn replay_from_eof() -> Vec<Vec<String>> {
    // Changing the pause flag at EOF leaves mpv on the last frame.
    vec![cmd(["seek", "0", "absolute+exact"]), set("pause", "no")]
}

pub(super) fn controller_speed(
    state: &Playback,
    speed: f64,
    resume: bool,
) -> Result<Vec<Vec<String>>, CommandError> {
    let mut commands = vec![Command::Speed(speed).mpv_args()?];
    if resume {
        if state.phase == Phase::Ended {
            commands.extend(replay_from_eof());
        } else {
            commands.push(set("pause", "no"));
        }
    }
    Ok(commands)
}

pub(super) fn save_resume_positions(enabled: bool) -> Vec<Vec<String>> {
    let value = flag(enabled);
    vec![
        set("save-position-on-quit", value),
        set("resume-playback", value),
    ]
}

pub(super) fn toggle_loop_mode(state: &Playback, requested: LoopMode) -> Vec<Vec<String>> {
    let mode = if state.loop_mode() == requested {
        LoopMode::Off
    } else {
        requested
    };
    set_loop_mode_commands(mode)
}

pub(super) fn set_loop_mode_commands(mode: LoopMode) -> Vec<Vec<String>> {
    match mode {
        LoopMode::Off => vec![set("loop-playlist", "no"), set("loop-file", "no")],
        LoopMode::File => vec![set("loop-file", "inf")],
        LoopMode::Playlist => vec![set("loop-playlist", "inf"), set("loop-file", "no")],
    }
}
