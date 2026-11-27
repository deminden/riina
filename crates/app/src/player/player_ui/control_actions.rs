//! Wire controls after the playback command sender and feature guards are ready.
use super::{
    PendingNavigation, audio_controls, catalog,
    context::{PlayerContext, bind},
    controller_arrows, delays, dialogs, embedded_input, feedback, hdr_output, input,
    playlist_actions, playlist_subtitles, subtitle_controls, transport, video_controls,
};
use crate::{
    PlayerWindow, crop_editor::Editor, platform, screenshot_output::Scratch, video_osd::VideoOsd,
};
use riina_core::{Command, audio_equalizer};
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

pub(super) struct Inputs<'a> {
    pub(super) context: &'a PlayerContext,
    pub(super) native: Option<&'a platform::NativePresentation>,
    pub(super) crop_editor: &'a Editor,
    pub(super) screenshot_scratch: Arc<Scratch>,
    pub(super) feedback_video_osd: Rc<RefCell<VideoOsd>>,
    pub(super) audio_equalizer: &'a Arc<Mutex<audio_equalizer::Settings>>,
    pub(super) pending_navigation: &'a PendingNavigation,
    pub(super) capture_transition_first: bool,
    pub(super) subtitle_assignments: playlist_subtitles::Assignments,
}

pub(super) fn install(app: &PlayerWindow, inputs: Inputs<'_>) {
    let Inputs {
        context,
        native,
        crop_editor,
        screenshot_scratch,
        feedback_video_osd,
        audio_equalizer,
        pending_navigation,
        capture_transition_first,
        subtitle_assignments,
    } = inputs;
    let command_sender = context.commands();
    crate::window_controls::install(app, native.map(platform::NativePresentation::host));
    crop_editor.install(app, command_sender.clone());
    video_controls::install_reflection(app, command_sender);
    feedback::install(app, command_sender, screenshot_scratch, feedback_video_osd);
    video_controls::install_deinterlace(app, command_sender);
    transport::install_pause(app, context);
    input::install_mpv_input_action(app, context);
    embedded_input::install(app, context);
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_mute,
        Command::ToggleMute
    );
    let hdr_player = command_sender.clone();
    hdr_output::install(app, move |command| {
        hdr_player.send(command).map_err(String::from)
    });
    transport::install_stop_and_steps(app, context, subtitle_assignments);
    playlist_actions::install_navigation(
        app,
        command_sender,
        pending_navigation,
        capture_transition_first,
    );
    transport::install_speed(app, context);
    video_controls::install(app, command_sender);
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_reset_subtitle_delay,
        Command::SubtitleDelay(0.0)
    );
    audio_controls::install(app, command_sender, audio_equalizer.clone());
    subtitle_controls::install(app, command_sender);
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_reset_audio_delay,
        Command::AudioDelay(0.0)
    );
    transport::install_loops(app, context);
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_secondary_subtitle_visibility,
        Command::ToggleSecondarySubtitleVisibility
    );
    catalog::install_track_cycles(app, context);
    audio_controls::install_audio_device(app, command_sender, context.playback());
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_subtitle_visibility,
        Command::ToggleSubtitleVisibility
    );
    subtitle_controls::install_encoding(app, command_sender);
    catalog::install_catalog_selection(app, context);
    delays::install_sliders(app, command_sender);
    playlist_actions::install(
        app,
        command_sender,
        pending_navigation,
        capture_transition_first,
    );
    dialogs::install_browse(app, context, native.map(platform::NativePresentation::host));
    transport::install_seek(app, command_sender);
    transport::install_timeline(app, context);
    controller_arrows::install(app, context.clone());
    dialogs::install_jump_time(app, context);
    dialogs::install_playlist_url(app, command_sender);
    audio_controls::install_volume(app, command_sender);
    transport::install_relative_speed(app, command_sender);
    audio_controls::install_volume_adjustment(app, command_sender);
}
