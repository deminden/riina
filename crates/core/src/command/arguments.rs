use super::Command;
use super::CommandError;
use crate::key_bindings;

impl Command {
    pub fn mpv_args(&self) -> Result<Vec<String>, CommandError> {
        let args: Vec<String> = match self {
            Self::MpvInputAction(action) => {
                key_bindings::mpv_action_args(action).map_err(CommandError::InputBinding)?
            }
            Self::PluginMpv(command) => command.arguments(),
            Self::ObserveMpvProperty(_) => {
                return invalid_state("mpv property observation is handled by the player worker");
            }
            Self::Load(_)
            | Self::LoadAtPosition { .. }
            | Self::SetPaused(_)
            | Self::TogglePause
            | Self::SetPlaybackEndPolicy { .. }
            | Self::Stop
            | Self::Speed(_)
            | Self::MultiplySpeed(_)
            | Self::FrameStep
            | Self::FrameBackStep
            | Self::CycleAbLoop
            | Self::ToggleFileLoop => self.playback_args()?,
            Self::Seek { .. }
            | Self::SeekWithType { .. }
            | Self::SelectChapter(_)
            | Self::AdjustChapter(_) => self.seek_args()?,
            Self::Volume(_)
            | Self::RestoreVolume(_)
            | Self::AudioDevice(_)
            | Self::ToggleMute
            | Self::SetMuted(_) => self.audio_args()?,
            Self::CaptureFrame
            | Self::CaptureThumbnailFrame
            | Self::CaptureTransitionFrame
            | Self::CaptureIncomingTransitionFrame
            | Self::CaptureMaterialFrame
            | Self::ScreenshotToFile { .. } => self.capture_args()?,
            Self::SelectTrack { .. }
            | Self::CycleTrack(_)
            | Self::SelectSecondarySubtitle(_)
            | Self::AddSubtitle(_)
            | Self::AddAudio(_)
            | Self::AddVideo(_) => self.tracks_args()?,
            Self::VideoAspect(_)
            | Self::ToggleVideoReflection(_)
            | Self::VideoRotation(_)
            | Self::Deinterlace(_)
            | Self::ToggleDeinterlace
            | Self::HardwareDecoding(_)
            | Self::VideoEqualizer { .. } => self.video_args()?,
            Self::PlaylistNext
            | Self::PlaylistPrevious
            | Self::SelectPlaylistEntry(_)
            | Self::RemovePlaylistEntry(_)
            | Self::MovePlaylistEntry { .. }
            | Self::ClearPlaylist => self.playlist_args()?,
            Self::MultiplySubtitleScale(_)
            | Self::SubtitleAssOverride { .. }
            | Self::AudioDelay(_)
            | Self::SubtitleDelay(_)
            | Self::ToggleSubtitleVisibility
            | Self::SubtitleFont(_)
            | Self::SubtitlePreferredLanguages(_)
            | Self::SubtitleBold(_)
            | Self::SubtitleItalic(_)
            | Self::SubtitleBlur(_)
            | Self::SubtitleSpacing(_)
            | Self::SubtitleShadowOffset(_)
            | Self::SubtitleTextColor(_)
            | Self::SubtitleBorderColor(_)
            | Self::SubtitleShadowColor(_)
            | Self::SubtitleOutlineSize(_)
            | Self::SubtitleFontSize(_)
            | Self::SubtitleScale(_)
            | Self::SecondarySubtitleDelay(_)
            | Self::SubtitlePosition(_)
            | Self::SecondarySubtitlePosition(_)
            | Self::SubtitleAlignX(_)
            | Self::SubtitleAlignY(_)
            | Self::SubtitleScaleWithWindow(_)
            | Self::SubtitleMarginX(_)
            | Self::SubtitleMarginY(_)
            | Self::ToggleSecondarySubtitleVisibility
            | Self::AdjustAudioDelay(_)
            | Self::AdjustSubtitleDelay(_)
            | Self::AdjustSubtitleScale(_)
            | Self::AdjustSubtitlePosition(_)
            | Self::SubtitleSeek(_) => self.subtitles_args()?,
            Self::OpenFiles(_)
            | Self::OpenPlaylist(_)
            | Self::OpenShuffledPlaylist(_)
            | Self::OpenMediaLink(_)
            | Self::OpenFilesWithSubtitles { .. }
            | Self::SetPauseWhenOpen(_)
            | Self::SetResumeLastPosition(_)
            | Self::Screenshot { .. }
            | Self::VideoCrop(_)
            | Self::VideoCropRectangle(_)
            | Self::RemoveDelogo
            | Self::ToggleVideoFilter(_)
            | Self::AddVideoFilter(_)
            | Self::RemoveVideoFilter(_)
            | Self::SetVideoFilterAt { .. }
            | Self::ToggleAudioFilter(_)
            | Self::AddAudioFilter(_)
            | Self::RemoveAudioFilter(_)
            | Self::SetAudioFilterAt { .. }
            | Self::VideoViewport(_)
            | Self::AudioEqualizer(_)
            | Self::SetReplayGain(_)
            | Self::VideoReflection { .. }
            | Self::EnterCrop(_)
            | Self::ExitCrop { .. }
            | Self::AdjustVolume { .. }
            | Self::SetVolumeBoost(_)
            | Self::AppendFiles(_)
            | Self::AddSubtitlesWithoutSelection(_)
            | Self::AddSubtitlesForMedia { .. }
            | Self::AddMatchedSubtitles { .. }
            | Self::InsertPlaylistFiles { .. }
            | Self::InsertPlaylistFilesBefore { .. }
            | Self::SubtitleLetterboxMargins(_)
            | Self::SubtitleEncoding(_)
            | Self::SubtitleAutoLoad(_)
            | Self::CycleLoop
            | Self::SetLoopMode(_)
            | Self::SetAbLoop { .. }
            | Self::SetAbLoopPoint { .. }
            | Self::SetToneMapping(_)
            | Self::SetIccProfileAuto(_)
            | Self::SetHdrEnabled(_)
            | Self::SetHdrEnabledWithIcc { .. }
            | Self::VideoThreads(_)
            | Self::SelectPlaylistId(_)
            | Self::RemovePlaylistId(_)
            | Self::RemovePlaylistIds(_)
            | Self::RemoveTrashedPlaylistEntries(_)
            | Self::QueuePlaylistIdsNext(_)
            | Self::MovePlaylistIdsBefore { .. }
            | Self::MovePlaylistId { .. }
            | Self::MovePlaylistIdBefore { .. } => {
                return invalid_state("This intent requires a command plan");
            }
            Self::Shutdown => return invalid_state("Shutdown is a lifecycle command"),
            _ => {
                return Err(CommandError::InvalidArgument {
                    what: "The requested value is out of range or non-finite",
                });
            }
        };
        Ok(args)
    }
}

fn invalid_state(reason: &'static str) -> Result<Vec<String>, CommandError> {
    Err(CommandError::InvalidState { reason })
}
