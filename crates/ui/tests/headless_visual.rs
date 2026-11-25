//! Pixel regressions for the generated components with a headless software renderer.
#![cfg(target_os = "windows")]

use i_slint_renderer_software::{MinimalSoftwareWindow, RepaintBufferType};
use riina_ui::{
    CatalogRow, LiveTextRegion, PlayerWindow, PreferencesSearchResult, PreferencesWindow,
};
use slint::{
    ComponentHandle, Image, LogicalPosition, PhysicalSize, PlatformError, Rgb8Pixel,
    SharedPixelBuffer, Window,
    platform::{Platform, WindowAdapter, WindowEvent},
};
use std::{error::Error, path::PathBuf, rc::Rc, time::Duration};

struct OffscreenPlatform;

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
    }

    fn duration_since_start(&self) -> Duration {
        Duration::ZERO
    }
}

struct Baselines {
    directory: PathBuf,
    update: bool,
    mismatch_directory: Option<PathBuf>,
    differences: Vec<String>,
}

impl Baselines {
    fn windows() -> Result<Self, Box<dyn Error>> {
        let directory = riina_test_support::workspace_root()?.join("tests/visual/headless/windows");
        let update =
            std::env::var_os("RIINA_UPDATE_HEADLESS_BASELINES").is_some_and(|value| value == "1");
        if update {
            std::fs::create_dir_all(&directory)?;
        }
        let mismatch_directory = std::env::var_os("RIINA_HEADLESS_MISMATCH_DIR").map(PathBuf::from);
        Ok(Self {
            directory,
            update,
            mismatch_directory,
            differences: Vec::new(),
        })
    }

    fn verify(
        &mut self,
        name: &str,
        window: &Window,
        size: PhysicalSize,
    ) -> Result<(), Box<dyn Error>> {
        window.set_size(size);
        slint::platform::update_timers_and_animations();
        let snapshot = window.take_snapshot()?;
        let path = self.directory.join(format!("{name}.png"));
        assert_eq!(
            (snapshot.width(), snapshot.height()),
            (size.width, size.height),
            "{name}"
        );
        let actual = snapshot.as_bytes();
        let (pixels, remainder) = actual.as_chunks::<4>();
        assert!(remainder.is_empty());
        assert!(
            pixels.iter().any(|pixel| pixel != &pixels[0]),
            "{name} rendered a uniform frame"
        );
        if self.update {
            image::save_buffer(
                &path,
                actual,
                size.width,
                size.height,
                image::ColorType::Rgba8,
            )?;
        } else {
            let comparison = match image::open(&path) {
                Ok(expected) => {
                    let expected = expected.into_rgba8();
                    if expected.dimensions() == (size.width, size.height) {
                        let (expected_pixels, _) = expected.as_raw().as_chunks::<4>();
                        let changed = expected_pixels
                            .iter()
                            .zip(pixels)
                            .filter(|(expected, actual)| expected != actual)
                            .count();
                        (changed != 0).then(|| format!("{changed} changed pixels"))
                    } else {
                        Some(format!(
                            "baseline is {} × {}, rendered {} × {}",
                            expected.width(),
                            expected.height(),
                            size.width,
                            size.height
                        ))
                    }
                }
                Err(error) => Some(format!("could not open baseline: {error}")),
            };
            if let Some(detail) = comparison {
                self.differences
                    .push(format!("{name}: {detail} ({})", path.display()));
                if let Some(directory) = &self.mismatch_directory {
                    std::fs::create_dir_all(directory)?;
                    image::save_buffer(
                        directory.join(format!("{name}.png")),
                        actual,
                        size.width,
                        size.height,
                        image::ColorType::Rgba8,
                    )?;
                }
            }
        }
        Ok(())
    }
    fn finish(self) {
        assert!(
            self.differences.is_empty(),
            "Visual baseline differences:\n{}",
            self.differences.join("\n")
        );
    }
}

#[derive(Clone, Copy)]
enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy)]
enum Layout {
    Standard,
    Compact,
    Docked,
    Music { chapters: bool },
    MusicResting,
}

fn player(
    state: PlaybackState,
    dark: bool,
    sidebars: (i32, i32),
    layout: Layout,
) -> Result<PlayerWindow, PlatformError> {
    let window = PlayerWindow::new()?;
    window
        .global::<riina_ui::PlayerAppearance>()
        .set_platform_windows(true);
    window.set_native_video(false);
    window
        .global::<riina_ui::PlayerPreferences>()
        .set_disable_animations(true);
    window
        .global::<riina_ui::PlayerAppearance>()
        .set_dark_theme(dark);
    window.set_engine_ready(true);
    window
        .global::<riina_ui::PlayerControlsState>()
        .set_controls_visible(true);
    window.set_loaded(!matches!(state, PlaybackState::Stopped));
    window.set_paused(!matches!(state, PlaybackState::Playing));
    window
        .global::<riina_ui::PlayerAppearance>()
        .set_compact_ui(matches!(layout, Layout::Compact));
    window
        .global::<riina_ui::PlayerLayoutState>()
        .set_docked_controls(matches!(layout, Layout::Docked));
    if matches!(layout, Layout::Docked) {
        window
            .global::<riina_ui::PlayerLayoutState>()
            .set_edge_to_edge_video(false);
        window
            .global::<riina_ui::PlayerLayoutState>()
            .set_osc_position(2);
    }
    if let Layout::Music { chapters } = layout {
        let music = window.global::<riina_ui::PlayerMusicModeState>();
        music.set_music_mode(true);
        music.set_music_mode_is_audio(true);
        music.set_music_mode_show_playlist(true);
        if chapters {
            window
                .global::<riina_ui::PlayerSidebarState>()
                .set_remembered_playlist_pane(3);
        }
    }
    if matches!(layout, Layout::MusicResting) {
        let music = window.global::<riina_ui::PlayerMusicModeState>();
        music.set_music_mode(true);
        music.set_music_mode_is_audio(true);
        let controls = window.global::<riina_ui::PlayerControlsState>();
        controls.set_controls_visible(false);
        controls.set_controls_rendered(false);
    }
    window
        .global::<riina_ui::PlayerSidebarState>()
        .set_sidebar(sidebars.0);
    window
        .global::<riina_ui::PlayerSidebarState>()
        .set_leading_sidebar(sidebars.1);

    if window.get_loaded() {
        window.set_media_title("Headless test clip".into());
        window
            .global::<riina_ui::PlaybackTimelineState>()
            .set_position(18.0);
        window
            .global::<riina_ui::PlaybackTimelineState>()
            .set_duration(72.0);
        window
            .global::<riina_ui::PlaybackTimelineState>()
            .set_position_label("0:18".into());
        window
            .global::<riina_ui::PlaybackTimelineState>()
            .set_duration_label("1:12".into());
        window
            .global::<riina_ui::PlaybackAudioState>()
            .set_has_active_audio_track(true);
        window
            .global::<riina_ui::PlaybackTimelineState>()
            .set_seekable(true);
        window.set_status("".into());
        window
            .global::<riina_ui::PlaybackAudioState>()
            .set_audio_tracks(catalog(&["Stereo", "Commentary"]));
        window
            .global::<riina_ui::PlaybackVideoState>()
            .set_video_tracks(catalog(&["Video track"]));
        window
            .global::<riina_ui::PlaybackSubtitleState>()
            .set_subtitle_tracks(catalog(&["English", "French"]));
        window
            .global::<riina_ui::PlayerCatalogState>()
            .set_playlist(catalog(&["Headless test clip", "Next clip", "Final clip"]));
        let chapters = [
            ("Opening", "00:00 → 00:25"),
            ("Middle", "00:25 → 00:51"),
            ("Credits", "00:51 → End"),
        ];
        window
            .global::<riina_ui::PlayerCatalogState>()
            .set_chapters(slint::ModelRc::new(slint::VecModel::from(
                chapters
                    .into_iter()
                    .enumerate()
                    .map(|(index, (title, range))| CatalogRow {
                        token: index.to_string().into(),
                        label: title.into(),
                        detail: range.into(),
                        selected: index == 0,
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            )));
        window
            .global::<riina_ui::PlayerRenderedVideo>()
            .set_video_frame(test_picture());
    }
    window.show()?;
    Ok(window)
}

fn catalog(labels: &[&str]) -> slint::ModelRc<CatalogRow> {
    slint::ModelRc::new(slint::VecModel::from(
        labels
            .iter()
            .enumerate()
            .map(|(index, label)| CatalogRow {
                token: (index + 1).to_string().into(),
                label: (*label).into(),
                selected: index == 0,
                playing: index == 0,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
}

fn test_picture() -> Image {
    let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(48, 27);
    for (i, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        let x = (i % 48) as u8;
        let y = (i / 48) as u8;
        *pixel = Rgb8Pixel::new(35 + 3 * x, 52 + 5 * y, 113);
    }
    Image::from_rgb8(pixels)
}

#[test]
fn player_and_settings_match_their_pixels() -> Result<(), Box<dyn Error>> {
    slint::platform::set_platform(Box::new(OffscreenPlatform))?;
    let mut baselines = Baselines::windows()?;
    player_frames(&mut baselines)?;
    player_overlay_frames(&mut baselines)?;
    settings_frames(&mut baselines)?;
    settings_overlay_frames(&mut baselines)?;
    baselines.finish();
    Ok(())
}

fn player_frames(baselines: &mut Baselines) -> Result<(), Box<dyn Error>> {
    use Layout::{Compact, Docked, Music, MusicResting, Standard};
    use PlaybackState::{Paused, Playing, Stopped};
    let cases = [
        ("stopped", Stopped, (0, 0), Standard),
        ("playing", Playing, (0, 0), Standard),
        ("paused", Paused, (0, 0), Standard),
        ("audio", Paused, (1, 0), Standard),
        ("subtitles", Paused, (2, 0), Standard),
        ("playlist", Paused, (4, 0), Standard),
        ("chapters", Paused, (3, 0), Standard),
        ("general", Paused, (5, 0), Standard),
        ("video", Paused, (6, 0), Standard),
        ("plugins", Paused, (7, 0), Standard),
        ("paired-sidebars", Paused, (4, 5), Standard),
        ("compact", Paused, (0, 0), Compact),
        ("docked", Playing, (0, 0), Docked),
        ("music", Playing, (0, 0), Music { chapters: false }),
        ("music-chapters", Paused, (0, 0), Music { chapters: true }),
        ("music-resting", Paused, (0, 0), MusicResting),
    ];
    for (theme, dark) in [("dark", true), ("light", false)] {
        for (name, state, sidebars, layout) in cases {
            let window = player(state, dark, sidebars, layout)?;
            let size = match layout {
                Music { .. } => {
                    let size = PhysicalSize::new(320, 480);
                    window.window().set_size(size);
                    window.window().dispatch_event(WindowEvent::PointerMoved {
                        position: LogicalPosition::new(12.0, 112.0),
                    });
                    size
                }
                MusicResting => PhysicalSize::new(300, 259),
                _ => PhysicalSize::new(1000, 640),
            };
            baselines.verify(&format!("player-{theme}-{name}"), window.window(), size)?;
            window.hide()?;
        }
    }
    Ok(())
}

fn player_overlay_frames(baselines: &mut Baselines) -> Result<(), Box<dyn Error>> {
    type OverlaySetup = fn(&PlayerWindow);
    let cases: [(&str, OverlaySetup); 6] = [
        ("player-seek-osd", |window| {
            window
                .global::<riina_ui::PlaybackTimelineState>()
                .set_seek_position_label("0:18".into());
            window
                .global::<riina_ui::PlaybackTimelineState>()
                .set_seek_duration_label("1:12".into());
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_seek_osd_visible(true);
        }),
        ("player-playback-osd", |window| {
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_video_osd_text("Volume: 64%".into());
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_video_osd_progress(0.64);
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_video_osd_visible(true);
        }),
        ("player-plugin-osd", |window| {
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_plugin_osd_message("Plugin message".into());
        }),
        ("player-fullscreen-info", |window| {
            window.set_fullscreen(true);
            window
                .global::<riina_ui::PlayerPreferences>()
                .set_display_time_and_battery_in_fullscreen(true);
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_fullscreen_info_time("16:45".into());
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_fullscreen_info_battery("82%".into());
            window
                .global::<riina_ui::PlayerOsdState>()
                .set_fullscreen_info_battery_level(82.0);
        }),
        ("player-crop-video", |window| {
            window
                .global::<riina_ui::PlayerCropState>()
                .set_crop_picture_x(24.0);
            window
                .global::<riina_ui::PlayerCropState>()
                .set_crop_picture_y(50.0);
            window
                .global::<riina_ui::PlayerCropState>()
                .set_crop_picture_width(800.0);
            window
                .global::<riina_ui::PlayerCropState>()
                .set_crop_picture_height(450.0);
            window
                .global::<riina_ui::PlayerCropState>()
                .set_crop_editing(true);
        }),
        ("player-live-text", |window| {
            window.set_live_text_enabled(true);
            window.set_live_text_selectable(true);
            window
                .global::<riina_ui::PlayerRenderedVideo>()
                .set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(vec![
                    LiveTextRegion {
                        text: "Selectable text".into(),
                        x: 0.24,
                        y: 0.32,
                        width: 0.30,
                        height: 0.08,
                    },
                ])));
        }),
    ];
    for (name, setup) in cases {
        let window = player(PlaybackState::Paused, true, (0, 0), Layout::Standard)?;
        window
            .global::<riina_ui::PlayerRenderedVideo>()
            .set_video_blurred_frame(test_picture());
        setup(&window);
        baselines.verify(name, window.window(), PhysicalSize::new(1000, 640))?;
        window.hide()?;
    }
    Ok(())
}

fn settings_frames(baselines: &mut Baselines) -> Result<(), Box<dyn Error>> {
    // Settings intentionally keep a light palette when the player uses a dark theme.
    let pages = [
        "general",
        "interface",
        "video",
        "audio",
        "subtitles",
        "network",
        "control",
        "keys",
        "advanced",
        "plugins",
        "utilities",
    ];
    for (index, name) in pages.into_iter().enumerate() {
        let window = PreferencesWindow::new()?;
        window.set_disable_animations(true);
        window.set_selected_pane(index as i32);
        window.show()?;
        baselines.verify(
            &format!("settings-{name}"),
            window.window(),
            PhysicalSize::new(750, 670),
        )?;
        window.hide()?;
    }
    Ok(())
}

fn settings_overlay_frames(baselines: &mut Baselines) -> Result<(), Box<dyn Error>> {
    type OverlaySetup = fn(&PreferencesWindow);
    let cases: [(&str, i32, OverlaySetup); 6] = [
        ("settings-search-popup", 0, |window| {
            window.set_search_query("volume".into());
            window.set_search_results(slint::ModelRc::new(slint::VecModel::from(vec![
                PreferencesSearchResult {
                    page_index: 3,
                    page_name: "Audio".into(),
                    title: "Volume".into(),
                    is_header: true,
                    ..Default::default()
                },
                PreferencesSearchResult {
                    page_index: 3,
                    page_name: "Audio".into(),
                    section_name: "Volume".into(),
                    title: "Initial volume".into(),
                    ..Default::default()
                },
            ])));
            window.set_selected_search_index(1);
        }),
        ("settings-key-mapping-dialog", 7, |window| {
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_editor_key("Ctrl+P".into());
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_editor_action("cycle pause".into());
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_editor_open(true);
        }),
        ("settings-key-profile-create-dialog", 7, |window| {
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_profile_editor_name("Viewing".into());
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_profile_editor_open(true);
        }),
        ("settings-key-profile-delete-dialog", 7, |window| {
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_profile_delete_confirm_open(true);
        }),
        ("settings-plugin-install-dialog", 9, |window| {
            window.set_plugin_github_repository("example/plugin".into());
            window.set_plugin_github_sheet_open(true);
        }),
        ("settings-default-app-dialog", 10, |window| {
            window
                .global::<riina_ui::UtilitySettingsState>()
                .set_utilities_default_audio(false);
            window
                .global::<riina_ui::UtilitySettingsState>()
                .set_utilities_default_app_sheet_open(true);
        }),
    ];
    for (name, page, setup) in cases {
        let window = PreferencesWindow::new()?;
        window.set_disable_animations(true);
        window.set_selected_pane(page);
        setup(&window);
        window.show()?;
        baselines.verify(name, window.window(), PhysicalSize::new(750, 670))?;
        window.hide()?;
    }
    Ok(())
}
