//! Profile and mapping actions for the Key Bindings pane.
use super::owner::Owner;
use crate::PreferencesWindow;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(super) fn connect(window: &PreferencesWindow, owner: &Owner) {
    install_selection(window, owner);
    install_profile_editor(window, owner);
    install_profile_files(window, owner);
    install_mapping_actions(window, owner);
}

fn install_selection(window: &PreferencesWindow, owner: &Owner) {
    let selected_profile_window = window.as_weak();
    let selected_profile_owner = owner.clone();
    window.on_key_binding_profile_selected(move |name| {
        let Some(window) = selected_profile_window.upgrade() else {
            return;
        };
        if let Err(error) = crate::key_binding_preferences::select_profile(&window, name.as_str()) {
            selected_profile_owner.report(error);
        }
    });
    let settings_refresh_window = window.as_weak();
    let settings_refresh_owner = owner.clone();
    window.on_key_binding_settings_changed(move || {
        let Some(window) = settings_refresh_window.upgrade() else {
            return;
        };
        let profile = window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_current_profile()
            .to_string();
        if let Err(error) = crate::key_binding_preferences::select_profile(&window, &profile) {
            settings_refresh_owner.report(error);
        }
    });
    let search_window = window.as_weak();
    let search_owner = owner.clone();
    window.on_key_binding_search_changed(move |query| {
        let Some(window) = search_window.upgrade() else {
            return;
        };
        if let Err(error) = crate::key_binding_preferences::search(&window, query.as_str()) {
            search_owner.report(error);
        }
    });
}

fn install_profile_editor(window: &PreferencesWindow, owner: &Owner) {
    let profile_action_window = window.as_weak();
    let profile_action_owner = owner.clone();
    window.on_key_binding_profile_created(move |name, mode| {
        let Some(window) = profile_action_window.upgrade() else {
            return;
        };
        let name = name.to_string();
        let source = window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_current_profile()
            .to_string();
        let result = (|| {
            let store = crate::key_binding_profiles::ProfileStore::for_current_user()
                .map_err(|error| error.to_string())?;
            match mode {
                0 => store.create(&name).map_err(|error| error.to_string())?,
                1 => {
                    let mappings = crate::key_binding_preferences::mappings_for(&source)?;
                    store
                        .create_with_mappings(&name, &mappings)
                        .map_err(|error| error.to_string())?;
                }
                2 => store
                    .rename(&source, &name)
                    .map_err(|error| error.to_string())?,
                _ => return Err("Choose a valid profile operation".to_owned()),
            }
            crate::key_binding_preferences::activate_profile(&window, &name)
        })();
        match result {
            Ok(()) => window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_profile_editor_open(false),
            Err(error) => {
                profile_action_owner.report(error);
            }
        }
    });
    let cancel_profile_window = window.as_weak();
    window.on_key_binding_profile_editor_cancelled(move || {
        if let Some(window) = cancel_profile_window.upgrade() {
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_profile_editor_open(false);
        }
    });
}

fn install_profile_files(window: &PreferencesWindow, owner: &Owner) {
    let import_window = window.as_weak();
    let import_owner = owner.clone();
    window.on_key_binding_import_profile_requested(move || {
        let Some(window) = import_window.upgrade() else {
            return;
        };
        let Some(dialog) = import_owner.begin_dialog(&window) else {
            return;
        };
        let report = import_owner.clone();
        let started = slint::spawn_local(async move {
            let result = import_profile(&window).await;
            drop(dialog);
            match result {
                Ok(Some(name)) => {
                    if let Err(error) =
                        crate::key_binding_preferences::activate_profile(&window, &name)
                    {
                        report.report(error);
                    }
                }
                Ok(None) => {}
                Err(error) => report.report(error),
            }
        });
        if let Err(error) = started {
            import_owner.report(error.to_string());
        }
    });
    let delete_window = window.as_weak();
    let delete_owner = owner.clone();
    window.on_key_binding_delete_profile(move || {
        let Some(window) = delete_window.upgrade() else {
            return;
        };
        let profile = window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_current_profile()
            .to_string();
        let result = crate::key_binding_profiles::ProfileStore::for_current_user()
            .and_then(|store| store.delete(&profile))
            .map_err(|error| error.to_string());
        if let Err(error) = result {
            delete_owner.report(error);
            return;
        }
        let mut settings = match crate::key_binding_preferences::current_window(&window) {
            Ok(settings) => settings,
            Err(error) => {
                delete_owner.report(error);
                return;
            }
        };
        settings.current_profile = "IINA Default".into();
        match crate::key_binding_preferences::refresh(&window, &settings) {
            Ok(()) => window.invoke_settings_changed(),
            Err(error) => {
                delete_owner.report(error);
            }
        }
    });
    let reveal_window = window.as_weak();
    let reveal_owner = owner.clone();
    window.on_key_binding_reveal_profile(move || {
        let Some(window) = reveal_window.upgrade() else {
            return;
        };
        let name = window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_current_profile()
            .to_string();
        let result = crate::key_binding_profiles::ProfileStore::for_current_user()
            .map_err(|error| error.to_string())
            .and_then(|store| {
                let profile = store.path_for(&name);
                if let Ok(path) = profile
                    && path.is_file()
                {
                    return crate::platform::reveal_file(&path).map_err(|error| error.to_string());
                }
                crate::platform::open_directory(store.directory())
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = result {
            reveal_owner.report(error);
        }
    });
}

async fn import_profile(window: &PreferencesWindow) -> Result<Option<String>, String> {
    let owner = window
        .window()
        .winit_window()
        .await
        .map_err(|error| error.to_string())?;
    let file = rfd::AsyncFileDialog::new()
        .set_title("Import key binding configuration")
        .add_filter("mpv input.conf", &["conf"])
        .set_parent(owner.as_ref())
        .pick_file()
        .await;
    let Some(file) = file else {
        return Ok(None);
    };
    crate::key_binding_profiles::ProfileStore::for_current_user()
        .and_then(|store| store.import(file.path()))
        .map(Some)
        .map_err(|error| error.to_string())
}

fn install_mapping_actions(window: &PreferencesWindow, owner: &Owner) {
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .on_build_binding_action(|command, selection, modifier, value| {
            riina_core::key_bindings::build_action(&command, &selection, &modifier, &value)
                .unwrap_or_default()
                .into()
        });
    let add_window = window.as_weak();
    window.on_key_binding_add_requested(move || {
        let Some(window) = add_window.upgrade() else {
            return;
        };
        if window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_profile_index()
            < 5
        {
            return;
        }
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_editor_row(-1);
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_editor_key("".into());
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_editor_action("".into());
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_editor_comment("".into());
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_editor_kind(0);
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_editor_open(true);
    });
    let edit_window = window.as_weak();
    let edit_owner = owner.clone();
    window.on_key_binding_edit_requested(move |row| {
        let Some(window) = edit_window.upgrade() else {
            return;
        };
        match crate::key_binding_preferences::mapping_for_row(&window, row) {
            Ok(mapping) => {
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_row(row);
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_key(mapping.key.into());
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_action(mapping.action.into());
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_comment(mapping.comment.unwrap_or_default().into());
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_kind(
                        if mapping.kind == riina_core::key_bindings::ActionKind::Riina {
                            1
                        } else {
                            0
                        },
                    );
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_open(true);
            }
            Err(error) => {
                edit_owner.report(error);
            }
        }
    });
    let key_window = window.as_weak();
    window.on_key_binding_key_recorded(move |text, control, alt, shift, meta| {
        if let Some(window) = key_window.upgrade()
            && let Some(key) = crate::key_binding_preferences::recorded_key(
                text.as_str(),
                control,
                alt,
                shift,
                meta,
            )
        {
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_editor_key(key.into());
        }
    });
    let remove_window = window.as_weak();
    let remove_owner = owner.clone();
    window.on_key_binding_remove_requested(move |row| {
        let Some(window) = remove_window.upgrade() else {
            return;
        };
        match crate::key_binding_preferences::remove_mapping(&window, row) {
            Ok(()) => window.invoke_settings_changed(),
            Err(error) => {
                remove_owner.report(error);
            }
        }
    });
    let submit_window = window.as_weak();
    let submit_owner = owner.clone();
    window.on_key_binding_mapping_submitted(move |key, action, comment, kind, row| {
        let Some(window) = submit_window.upgrade() else {
            return;
        };
        match crate::key_binding_preferences::save_mapping(
            &window,
            row,
            key.as_str(),
            action.as_str(),
            comment.as_str(),
            kind,
        ) {
            Ok(()) => {
                window
                    .global::<riina_ui::KeyBindingSettingsState>()
                    .set_key_binding_editor_open(false);
                window.invoke_key_binding_settings_changed();
                window.invoke_settings_changed();
            }
            Err(error) => {
                submit_owner.report(error);
            }
        }
    });
    let cancel_window = window.as_weak();
    window.on_key_binding_editor_cancelled(move || {
        if let Some(window) = cancel_window.upgrade() {
            window
                .global::<riina_ui::KeyBindingSettingsState>()
                .set_key_binding_editor_open(false);
        }
    });
}
