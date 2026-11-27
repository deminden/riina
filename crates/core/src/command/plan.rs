use super::{
    Command, CommandError, CommandPlan, audio, filters, flag, network, playback, playlist,
    screenshot_plan, seek, set, subtitles, video,
};
use crate::{Phase, Playback, hdr_preferences, key_bindings, video_viewport};

impl Command {
    pub fn plan(&self, state: &Playback) -> Result<CommandPlan, CommandError> {
        self.plan_with_playlist_shuffle(state, state.playlist_shuffled)
    }

    /// Use the shuffle state expected after earlier submissions to the same engine.
    pub fn plan_with_playlist_shuffle(
        &self,
        state: &Playback,
        playlist_shuffled: bool,
    ) -> Result<CommandPlan, CommandError> {
        self.validate_capture(state)?;
        let commands = match self {
            Self::ObserveMpvProperty(property) if valid_mpv_property_name(property) => Vec::new(),
            Self::ObserveMpvProperty(_) => {
                return Err(CommandError::InvalidArgument {
                    what: "Invalid mpv property name",
                });
            }
            Self::MpvInputAction(action) => key_bindings::mpv_action_native_commands(action)
                .map_err(CommandError::InputBinding)?,
            Self::MpvScriptKeys { keys, .. } => key_bindings::script_keypresses(keys)?,
            Self::AudioEqualizer(gains) => audio::equalizer(state, gains)?,
            Self::SetReplayGain(options) => audio::replay_gain(*options)?,
            Self::SubtitleLetterboxMargins(enabled) => {
                let value = flag(*enabled);
                vec![
                    set("sub-use-margins", value),
                    set("sub-ass-force-margins", value),
                ]
            }
            Self::SubtitleEncoding(encoding) => subtitles::encoding(encoding)?,
            Self::TogglePause | Self::SetPaused(false) if state.phase == Phase::Ended => {
                playback::replay_from_eof()
            }
            Self::ControllerSpeed { speed, resume } => {
                playback::controller_speed(state, *speed, *resume)?
            }
            Self::NetworkCache(preferences) => {
                preferences.commands().map_err(CommandError::Preference)?
            }
            Self::NetworkOnlineVideos(preferences) => network::online_videos(preferences)?,
            Self::Screenshot { .. } | Self::ScreenshotBinding { .. } => {
                screenshot_plan::plan(self)?
            }
            Self::VideoViewport(viewport) => video_viewport::Viewport::commands(*viewport),
            Self::VideoReflection { axis, enabled } => axis.commands(*enabled, state)?,
            Self::EnterCrop(session) => session.enter_commands(state)?,
            Self::ExitCrop { session, rectangle } => session.exit_commands(state, *rectangle)?,
            Self::RemoveDelogo => video::remove_delogo(state)?,
            Self::VideoCropRectangle(rectangle) => video::crop_rectangle(state, rectangle)?,
            Self::VideoCrop(preset) => video::crop_preset(state, *preset)?,
            Self::AdjustVolume { delta, boost } => audio::adjust_volume(*delta, *boost)?,
            Self::SetVolumeBoost(boost) => audio::volume_adjustment(0.0, *boost),
            Self::SetPauseWhenOpen(_) => Vec::new(),
            Self::Stop(_) => playback::stop(state),
            Self::SetResumeLastPosition(enabled) => playback::save_resume_positions(*enabled),
            Self::SetAbLoop { a, b } => seek::set_loop_points(*a, *b)?,
            Self::SetAbLoopPoint { point, value } => seek::set_loop_point(*point, *value)?,
            Self::SetToneMapping(settings) => settings
                .engine_commands()
                .map_err(CommandError::Preference)?,
            Self::SetIccProfileAuto(enabled) => vec![set("icc-profile-auto", flag(*enabled))],
            Self::SetHdrEnabledWithIcc {
                enabled,
                load_icc_profile,
            } => hdr_preferences::output_commands_with_icc(
                *enabled,
                video::hdr_primaries(state, *enabled)?,
                *load_icc_profile,
            )?,
            Self::VideoThreads(threads) if *threads <= i32::MAX as u32 => {
                vec![set("vd-lavc-threads", threads.to_string())]
            }
            Self::VideoThreads(_) => {
                return Err(CommandError::InvalidArgument {
                    what: "Video decoder threads must be between 0 and 2,147,483,647",
                });
            }
            Self::TogglePlaylistShuffle => vec![playlist::shuffle_command(!playlist_shuffled)],
            Self::SetPlaylistShuffle(enabled) if *enabled == playlist_shuffled => Vec::new(),
            Self::SetPlaylistShuffle(enabled) => vec![playlist::shuffle_command(*enabled)],
            Self::SortPlaylist(sort) => playlist::reorder(&state.playlist, *sort)?,
            Self::SelectPlaylistId(id)
            | Self::RemovePlaylistId(id)
            | Self::MovePlaylistId { id, .. }
            | Self::MovePlaylistIdBefore { id, .. } => {
                return self.plan_playlist_id(state, *id);
            }
            Self::RemovePlaylistIds(_)
            | Self::RemoveDeletedPlaylistEntries(_)
            | Self::QueuePlaylistIdsNext(_)
            | Self::MovePlaylistIdsBefore { .. } => playlist::edit_selection(state, self)?,
            Self::OpenFiles(paths)
            | Self::OpenPlaylist(paths)
            | Self::OpenShuffledPlaylist(paths)
            | Self::AppendFiles(paths)
            | Self::OpenFilesWithSubtitles { paths, .. } => playlist::open_files(self, paths)?,
            Self::InsertPlaylistFiles { paths, index } => {
                playlist::insert_files(state, paths, *index)?
            }
            Self::InsertPlaylistTrack { path, after_id, .. } => {
                playlist::insert_track(state, path, *after_id)?
            }
            Self::InsertPlaylistFilesBefore { paths, before_id } => {
                playlist::insert_files_before(state, paths, *before_id)?
            }
            // A discovery result already queued during replacement still reaches the worker.
            Self::InsertPlaylistFilesAroundCurrent { .. }
                if state.explicit_playlist || !state.folder_discovery_allowed =>
            {
                Vec::new()
            }
            Self::InsertPlaylistFilesAroundCurrent { before, after } => {
                playlist::insert_files_around_current(state, before, after)?
            }
            Self::Load(_) | Self::LoadAtPosition { .. } => playback::load(self)?,
            Self::OpenMediaLink(link) => super::media_link::plan(link)?,
            Self::SelectChapter(index) | Self::ActivateChapter { index, .. } => {
                seek::select_chapter(state, *index, self)?
            }
            Self::SeekTimeline { .. } if !state.timeline_seekable() => Vec::new(),
            Self::SelectPlaylistEntry(_)
            | Self::PlaylistNext
            | Self::PlaylistPrevious
            | Self::RemovePlaylistEntry(_)
            | Self::MovePlaylistEntry { .. } => return self.plan_playlist_navigation(state),
            Self::AddSubtitle(_)
            | Self::AddSubtitlesForMedia { .. }
            | Self::AddSubtitlesWithoutSelection(_)
            | Self::AddMatchedSubtitles { .. } => subtitles::load(state, self)?,
            Self::CycleLoop => playback::set_loop_mode_commands(state.loop_mode().next()),
            Self::ToggleLoopMode(mode) => playback::toggle_loop_mode(state, *mode),
            Self::SetLoopMode(mode) => playback::set_loop_mode_commands(*mode),
            _ => match filters::plan_edit(self, state) {
                Some(edit) => edit?,
                None => vec![self.mpv_args()?],
            },
        };
        Ok(CommandPlan {
            commands,
            resume_on_load: playback::resume_on_open(self),
        })
    }
}
fn valid_mpv_property_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && !name.contains('.')
        && !name.chars().any(char::is_control)
}
