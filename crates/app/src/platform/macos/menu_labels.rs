//! Captions shared by the Slint menu and the AppKit menu owners.
use riina_ui::{ApplicationMenuText, PlayerMenuWindow};
use slint::{ComponentHandle, SharedString};

macro_rules! labels {
    ($($field:ident => $getter:ident, $english:literal;)*) => {
        pub(crate) struct MenuLabels {
            $(pub $field: SharedString,)*
            pub reveal_in_finder: SharedString,
        }

        impl MenuLabels {
            pub(crate) fn new() -> Self {
                let component = PlayerMenuWindow::new()
                    .inspect_err(|error| {
                        crate::diagnostics::warn!("Could not initialize the menu text: {error}");
                    })
                    .ok();
                let catalog = component.as_ref().map(|view| view.global::<ApplicationMenuText>());
                Self {
                    $($field: catalog.as_ref().map_or_else(
                         || $english.into(),
                         ApplicationMenuText::$getter,
                    ),)*
                    reveal_in_finder: catalog.as_ref().map_or_else(
                        || "Show Current File in Finder".into(),
                        |labels| labels.invoke_reveal_current("Finder".into()),
                    ),
                }
            }
        }
    };
}

labels! {
    file_title => get_file_title, "File";
    edit_title => get_edit_title, "Edit";
    plugin_title => get_plugin_title, "Plugin";
    window_title => get_window_title, "Window";
    help_title => get_help_title, "Help";
    services_title => get_services_title, "Services";
    about => get_about, "About Riina";
    check_updates => get_check_updates, "Check for Updates…";
    settings => get_settings, "Settings…";
    preferences => get_preferences, "Preferences…";
    hide => get_hide, "Hide Riina";
    hide_others => get_hide_others, "Hide Others";
    show_all => get_show_all, "Show All";
    quit => get_quit, "Quit Riina";
    open => get_open, "Open…";
    open_in_new_window => get_open_in_new_window, "Open in New Window…";
    open_url => get_open_url, "Open URL…";
    open_url_in_new_window => get_open_url_in_new_window, "Open URL in New Window…";
    open_recent => get_open_recent, "Open Recent";
    clear_recent => get_clear_recent, "Clear Menu";
    new_window => get_new_window, "New Window";
    playback_history => get_playback_history, "Playback History";
    delete_current => get_delete_current, "Delete Current File";
    save_current_playlist => get_save_current_playlist, "Save Current Playlist…";
    close => get_close, "Close";
    undo => get_undo, "Undo";
    redo => get_redo, "Redo";
    cut => get_cut, "Cut";
    copy => get_copy, "Copy";
    paste => get_paste, "Paste";
    delete => get_delete, "Delete";
    select_all => get_select_all, "Select All";
    transformations => get_transformations, "Transformations";
    uppercase => get_uppercase, "Make Upper Case";
    lowercase => get_lowercase, "Make Lower Case";
    capitalize => get_capitalize, "Capitalize";
    speech => get_speech, "Speech";
    start_speaking => get_start_speaking, "Start Speaking";
    stop_speaking => get_stop_speaking, "Stop Speaking";
    manage_plugins => get_manage_plugins, "Manage Plugins…";
    show_plugin_panel => get_show_plugin_panel, "Show Plugins Panel";
    more => get_more, "More…";
    developer_tool => get_developer_tool, "Developer Tool";
    reload_plugins => get_reload_plugins, "Reload All Plugins";
    minimize => get_minimize, "Minimize";
    zoom => get_zoom, "Zoom";
    log_viewer => get_log_viewer, "Log Viewer";
    bring_to_front => get_bring_to_front, "Bring All to Front";
    help => get_help, "Riina Help";
    dump_debug => get_dump_debug, "Dump Debug Info";
    release_highlights => get_release_highlights, "Release Highlights";
    website => get_website, "Website";
}
