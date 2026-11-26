use crate::*;

macro_rules! command_samples {
    ($($variant:ident $([$($argument:expr),+])? $({$($field:ident: $value:expr),+ $(,)?})?;)+) => {
        fn all_commands_for_snapshot() -> Vec<Command> {
            vec![$(Command::$variant $(($($argument),+))? $({$($field: $value),+})?),+]
        }

        fn command_name(command: &Command) -> &'static str {
            match command {
                $(command_samples!(@pattern $variant $([$($argument),+])? $({$($field),+})?)
                    => stringify!($variant),)+
            }
        }
    };
    (@pattern $variant:ident [$($argument:expr),+]) => { Command::$variant(..) };
    (@pattern $variant:ident {$($field:ident),+}) => { Command::$variant { .. } };
    (@pattern $variant:ident) => { Command::$variant };
}

// Each row also generates a match arm. Adding a Command variant requires
// supplying a sample here, and changing a plan requires reviewing its golden row.
command_samples! {
    Load["first.mkv".into()];
    LoadAtPosition { path: "first.mkv".into(), position: 21.5 };
    OpenFiles[vec!["first.mkv".into(), "second.mkv".into()]];
    OpenPlaylist[vec!["first.mkv".into(), "second.mkv".into()]];
    OpenShuffledPlaylist[vec!["first.mkv".into(), "second.mkv".into()]];
    OpenMediaLink[MediaLink {
        path: "https://example.test/media.mkv".into(), append: false,
        start: Some(5.0), pause: Some(true), volume: Some(40.0), speed: Some(1.25), mute: Some(false),
    }];
    OpenFilesWithSubtitles { paths: vec!["first.mkv".into()], subtitles: vec!["dialogue.srt".into()] };
    SetPauseWhenOpen[true];
    SetPlaybackEndPolicy { keep_open_on_file_end: true, playlist_auto_play_next: false };
    SetResumeLastPosition[true];
    SetPaused[false];
    TogglePause;
    MpvInputAction["seek 5; set pause yes".into()];
    MpvScriptKeys { owner: key_bindings::MpvInputOwner::Console, keys: vec!["É".into(), "ENTER".into()] };
    PluginMpv[sample_plugin_command()];
    Stop;
    Seek { seconds: 4.25, absolute: false };
    SeekWithType { seconds: 31.5, seek_type: control_preferences::SeekType::Exact };
    Volume[64.0];
    RestoreVolume[64.0];
    AudioDevice["auto".into()];
    SetReplayGain[audio_preferences::ReplayGainOptions::default()];
    AdjustVolume { delta: 50.0, boost: true };
    SetVolumeBoost[true];
    ToggleMute;
    SetMuted[true];
    Speed[1.5];
    MultiplySpeed[1.25];
    FrameStep;
    FrameBackStep;
    CaptureFrame;
    CaptureThumbnailFrame;
    CaptureTransitionFrame;
    CaptureIncomingTransitionFrame;
    CaptureMaterialFrame;
    ObserveMpvProperty["time-pos".into()];
    Screenshot {
        directory: "screenshots".into(),
        content: screenshot::Content::Subtitles,
        encoding: screenshot::Encoding::default(),
    };
    ScreenshotBinding {
        directory: "screenshots".into(),
        default_content: screenshot::Content::Subtitles,
        encoding: screenshot::Encoding::default(),
        binding: screenshot::Binding::parse("screenshot window").unwrap(),
    };
    ScreenshotToFile { path: "still.png".into(), content: screenshot::Content::Video };
    VideoAspect[video_controls::AspectRatio::from_text("4:3")];
    VideoRotation[video_controls::Rotation::Clockwise90];
    Deinterlace[true];
    ToggleDeinterlace;
    VideoCrop[Some(crop::Preset::FourThree)];
    VideoCropRectangle[sample_rectangle()];
    RemoveDelogo;
    ToggleVideoFilter["hflip".into()];
    AddVideoFilter["vflip".into()];
    RemoveVideoFilter[sample_filter("hflip", None)];
    SetVideoFilterAt { index: 0, filter: "vflip".into() };
    ToggleAudioFilter["acompressor".into()];
    AddAudioFilter["aecho".into()];
    RemoveAudioFilter[sample_filter("acompressor", None)];
    SetAudioFilterAt { index: 0, filter: "aecho".into() };
    ToggleVideoReflection[video_reflection::Axis::Horizontal];
    VideoViewport[Some(sample_viewport())];
    VideoReflection { axis: video_reflection::Axis::Vertical, enabled: true };
    EnterCrop[sample_crop_session()];
    ExitCrop { session: sample_crop_session(), rectangle: Some(sample_rectangle()) };
    HardwareDecoding[video_controls::HardwareDecoder::AutoCopy];
    VideoThreads[8];
    SetToneMapping[hdr_preferences::Settings::default()];
    SetIccProfileAuto[true];
    SetHdrEnabled[true];
    SetHdrEnabledWithIcc { enabled: true, load_icc_profile: false };
    SubtitleAssOverride {
        lane: subtitle_controls::SubtitleLane::Secondary,
        level: subtitle_controls::AssOverrideLevel::Force,
    };
    SubtitleAlignX[subtitle_controls::HorizontalAlignment::Left];
    SubtitleAlignY[subtitle_controls::VerticalAlignment::Top];
    SubtitleLetterboxMargins[true];
    SubtitleScaleWithWindow[true];
    SubtitleMarginX[20.0];
    SubtitleMarginY[25.0];
    SubtitlePreferredLanguages["eng,fra".into()];
    SubtitleEncoding["SHIFT-JIS".into()];
    SubtitleAutoLoad[subtitle_controls::AutoLoadPreferences::default()];
    NetworkCache[network_preferences::Cache::default()];
    NetworkOnlineVideos[network_preferences::OnlineVideos::default()];
    VideoEqualizer { option: video_controls::Equalizer::Brightness, value: 12 };
    PlaylistNext;
    PlaylistPrevious;
    TogglePlaylistShuffle;
    SortPlaylist[playlist::PlaylistSort::FilenameAscending];
    SelectTrack { kind: TrackKind::Audio, id: Some(2) };
    CycleTrack[TrackKind::Audio];
    SelectSecondarySubtitle[Some(3)];
    SelectChapter[1];
    AdjustChapter[1];
    SelectPlaylistEntry[1];
    SelectPlaylistId[20];
    RemovePlaylistId[20];
    RemovePlaylistIds[vec![20, 30]];
    RemoveDeletedPlaylistEntries[vec![
        PlaylistFileReference { id: 10, filename: "b.mkv".into() },
        PlaylistFileReference { id: 20, filename: "a.mkv".into() },
        PlaylistFileReference { id: 30, filename: "old-c.mkv".into() },
        PlaylistFileReference { id: 40, filename: "removed.mkv".into() },
    ]];
    QueuePlaylistIdsNext[vec![10, 30]];
    MovePlaylistIdsBefore { ids: vec![10, 20], before_id: None };
    MovePlaylistId { id: 20, direction: -1 };
    MovePlaylistIdBefore { id: 10, before_id: 30 };
    AppendFiles[vec!["bonus.mkv".into()]];
    InsertPlaylistFiles { paths: vec!["bonus.mkv".into()], index: 1 };
    InsertPlaylistFilesBefore { paths: vec!["bonus.mkv".into()], before_id: Some(20) };
    InsertPlaylistFilesAroundCurrent { before: vec!["intro.mkv".into()], after: vec!["outro.mkv".into()] };
    RemovePlaylistEntry[1];
    MovePlaylistEntry { from: 0, to: 2 };
    ClearPlaylist;
    AddSubtitle["dialogue.srt".into()];
    AddSubtitlesForMedia { generation: 0, paths: vec!["dialogue.srt".into(), "translation.srt".into()] };
    AddSubtitlesWithoutSelection[vec!["dialogue.srt".into(), "translation.srt".into()]];
    AddMatchedSubtitles {
        generation: 0, paths: vec!["dialogue.srt".into(), "translation.srt".into()], select_first: true,
    };
    AddAudio["commentary.flac".into()];
    AddVideo["alternate.mkv".into()];
    AudioDelay[0.375];
    AudioEqualizer[[1.5; 10]];
    AdjustAudioDelay[-0.125];
    SubtitleDelay[0.5];
    SecondarySubtitleDelay[-0.25];
    SecondarySubtitlePosition[80.0];
    SubtitlePosition[95.0];
    SubtitleScale[1.25];
    MultiplySubtitleScale[1.125];
    SubtitleFontSize[48.0];
    SubtitleFont["Sans Serif".into()];
    SubtitleOutlineSize[2.5];
    SubtitleTextColor[sample_color()];
    SubtitleBorderColor[sample_color()];
    SubtitleShadowColor[sample_color()];
    SubtitleBold[true];
    SubtitleItalic[true];
    SubtitleBlur[0.75];
    SubtitleSpacing[1.5];
    SubtitleShadowOffset[1.25];
    AdjustSubtitleDelay[-0.125];
    AdjustSubtitleScale[0.25];
    AdjustSubtitlePosition[-1.0];
    SubtitleSeek[1];
    ToggleSubtitleVisibility;
    ToggleSecondarySubtitleVisibility;
    CycleLoop;
    ToggleFileLoop;
    SetLoopMode[LoopMode::Playlist];
    CycleAbLoop;
    SetAbLoop { a: Some(11.5), b: Some(30.25) };
    SetAbLoopPoint { point: AbLoopPoint::B, value: Some(34.0) };
    Shutdown;
}

fn sample_plugin_command() -> PluginMpvCommand {
    PluginMpvCommand::new("show-text", vec!["literal; stop".into()])
        .expect("The show-text sample is within the plugin capability")
}

fn sample_filter(name: &str, label: Option<&str>) -> crop::VideoFilter {
    crop::VideoFilter {
        name: name.into(),
        label: label.map(str::to_owned),
        enabled: Some(true),
        params: Default::default(),
    }
}

fn sample_rectangle() -> crop::Rectangle {
    crop::Rectangle {
        x: 20,
        y: 10,
        width: 600,
        height: 340,
    }
}

fn sample_viewport() -> video_viewport::Viewport {
    video_viewport::Viewport::new(
        (640.0, 400.0),
        crop_geometry::DisplayRect {
            x: 0.0,
            y: 20.0,
            width: 640.0,
            height: 360.0,
        },
    )
    .unwrap()
}

fn sample_crop_session() -> crop_session::Session {
    crop_session::Session::begin(&snapshot_state()).unwrap()
}

fn sample_color() -> subtitle_color::SubtitleColor {
    subtitle_color::SubtitleColor::from_rgba([0.25, 0.5, 0.75, 1.0]).unwrap()
}

fn snapshot_state() -> Playback {
    Playback {
        folder_discovery_allowed: true,
        phase: Phase::Paused,
        paused: true,
        save_position_on_quit: Some(true),
        position: Some(12.5),
        duration: Some(120.0),
        seekable: true,
        volume: 68.0,
        speed: 1.25,
        video: Some(VideoParameters {
            pixelformat: None,
            w: 640,
            h: 360,
            dw: None,
            dh: None,
            rotate: None,
            colormatrix: None,
            colorlevels: None,
            primaries: None,
            gamma: None,
            max_luma: None,
            min_luma: None,
            signal_peak: None,
        }),
        video_filters: Some(vec![
            sample_filter("hflip", None),
            sample_filter("lavfi", Some(delogo::FILTER_LABEL)),
        ]),
        audio_filters: Some(vec![sample_filter("acompressor", None)]),
        audio_equalizer: Some([1.0; 10]),
        tracks: serde_json::from_str::<Vec<_>>(
            r#"[{"id":1,"type":"video"},{"id":2,"type":"audio"},{"id":3,"type":"sub"}]"#,
        )
        .unwrap()
        .into(),
        chapters: serde_json::from_str::<Vec<_>>(r#"[{"time":0.0},{"time":45.0}]"#)
            .unwrap()
            .into(),
        playlist: serde_json::from_str::<Vec<_>>(
            r#"[
                {"id":10,"filename":"b.mkv","current":true,"playing":true},
                {"id":20,"filename":"a.mkv"},
                {"id":30,"filename":"c.mkv"}
            ]"#,
        )
        .unwrap()
        .into(),
        ab_loop_a: Some(9.0),
        ..Playback::default()
    }
}

#[test]
fn every_command_variant_keeps_its_native_plan() {
    let state = snapshot_state();
    let commands = all_commands_for_snapshot();
    // The trailing screenshot directory separator is native to the build OS.
    let separator = if cfg!(windows) { r"\\" } else { "/" };
    let expected = include_str!("fixtures/command-plans.jsonl").replace("<separator>", separator);
    assert_eq!(expected.lines().count(), commands.len());
    for (command, expected_row) in commands.into_iter().zip(expected.lines()) {
        let name = command_name(&command);
        let row = match command.plan(&state) {
            Ok(plan) => serde_json::json!({
                "command": name,
                "mpv": plan.commands,
                "resume_on_load": plan.resume_on_load,
            }),
            Err(error) => serde_json::json!({ "command": name, "error": error.to_string() }),
        };
        assert_eq!(row.to_string(), expected_row, "{name}");
    }
}

#[test]
fn worker_routes_every_command_to_the_expected_lifecycle() {
    let replacing = [
        "Load",
        "LoadAtPosition",
        "OpenFiles",
        "OpenPlaylist",
        "OpenShuffledPlaylist",
        "OpenMediaLink",
        "OpenFilesWithSubtitles",
        "Stop",
    ];
    let decoder_changes = [
        "HardwareDecoding",
        "VideoAspect",
        "VideoRotation",
        "Deinterlace",
        "ToggleDeinterlace",
        "VideoCrop",
        "VideoCropRectangle",
        "VideoReflection",
        "ToggleVideoReflection",
        "EnterCrop",
        "ExitCrop",
    ];
    let shuffle_reset = [
        "SortPlaylist",
        "MovePlaylistEntry",
        "MovePlaylistId",
        "MovePlaylistIdBefore",
        "RemovePlaylistEntry",
        "RemovePlaylistId",
        "RemovePlaylistIds",
        "RemoveDeletedPlaylistEntries",
        "QueuePlaylistIdsNext",
        "MovePlaylistIdsBefore",
        "ClearPlaylist",
        "AppendFiles",
        "InsertPlaylistFiles",
        "InsertPlaylistFilesBefore",
        "OpenFiles",
        "OpenPlaylist",
        "OpenMediaLink",
        "OpenFilesWithSubtitles",
    ];
    for command in all_commands_for_snapshot() {
        let name = command_name(&command);
        assert_eq!(
            command.replaces_media(),
            replacing.contains(&name),
            "{name}"
        );
        assert_eq!(
            command.restarts_decoder(),
            replacing.contains(&name) || decoder_changes.contains(&name),
            "{name}"
        );
        assert_eq!(
            command.resets_playlist_shuffle(),
            shuffle_reset.contains(&name),
            "{name}"
        );
    }
}
