//! Profile and mapping actions for the Key Bindings pane.
use crate::{PlayerWindow, PreferencesWindow};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(super) fn connect(app: &PlayerWindow, window: &PreferencesWindow) {
    let selected_profile_window = window.as_weak();
    let selected_profile_parent = app.as_weak();
    window.on_key_binding_profile_selected(move |name| {
        let Some(window) = selected_profile_window.upgrade() else {
            return;
        };
        if let Err(error) = crate::key_binding_preferences::select_profile(&window, name.as_str())
            && let Some(app) = selected_profile_parent.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let settings_refresh_window = window.as_weak();
    let settings_refresh_parent = app.as_weak();
    window.on_key_binding_settings_changed(move || {
        let Some(window) = settings_refresh_window.upgrade() else {
            return;
        };
        let profile = window.get_key_binding_current_profile().to_string();
        if let Err(error) = crate::key_binding_preferences::select_profile(&window, &profile)
            && let Some(app) = settings_refresh_parent.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let search_window = window.as_weak();
    let search_parent = app.as_weak();
    window.on_key_binding_search_changed(move |query| {
        let Some(window) = search_window.upgrade() else {
            return;
        };
        if let Err(error) = crate::key_binding_preferences::search(&window, query.as_str())
            && let Some(app) = search_parent.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let profile_action_window = window.as_weak();
    let profile_action_parent = app.as_weak();
    window.on_key_binding_profile_created(move |name, mode| {
        let Some(window) = profile_action_window.upgrade() else {
            return;
        };
        let name = name.to_string();
        let source = window.get_key_binding_current_profile().to_string();
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
            Ok(()) => window.set_key_binding_profile_editor_open(false),
            Err(error) => {
                if let Some(app) = profile_action_parent.upgrade() {
                    app.set_status(error.into());
                }
            }
        }
    });
    let cancel_profile_window = window.as_weak();
    window.on_key_binding_profile_editor_cancelled(move || {
        if let Some(window) = cancel_profile_window.upgrade() {
            window.set_key_binding_profile_editor_open(false);
        }
    });
    let import_window = window.as_weak();
    let import_parent = app.as_weak();
    window.on_key_binding_import_profile_requested(move || {
        let (Some(window), Some(app)) = (import_window.upgrade(), import_parent.upgrade()) else {
            return;
        };
        if app.get_dialog_open() {
            return;
        }
        app.set_dialog_open(true);
        app.invoke_activity();
        let weak_window = window.as_weak();
        let weak_app = app.as_weak();
        let started = slint::spawn_local(async move {
            let result = async {
                let Some(window) = weak_window.upgrade() else {
                    return Ok(None);
                };
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
            .await;
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            app.set_dialog_open(false);
            app.invoke_activity();
            match result {
                Ok(Some(name)) => {
                    if let Some(window) = weak_window.upgrade()
                        && let Err(error) =
                            crate::key_binding_preferences::activate_profile(&window, &name)
                    {
                        app.set_status(error.into());
                    }
                }
                Ok(None) => {}
                Err(error) => app.set_status(error.into()),
            }
        });
        if let Err(error) = started {
            app.set_dialog_open(false);
            app.invoke_activity();
            app.set_status(error.to_string().into());
        }
    });
    let delete_window = window.as_weak();
    let delete_parent = app.as_weak();
    window.on_key_binding_delete_profile(move || {
        let Some(window) = delete_window.upgrade() else {
            return;
        };
        let profile = window.get_key_binding_current_profile().to_string();
        let result = crate::key_binding_profiles::ProfileStore::for_current_user()
            .and_then(|store| store.delete(&profile))
            .map_err(|error| error.to_string());
        if let Err(error) = result {
            if let Some(app) = delete_parent.upgrade() {
                app.set_status(error.into());
            }
            return;
        }
        let mut settings = match crate::key_binding_preferences::current_window(&window) {
            Ok(settings) => settings,
            Err(error) => {
                if let Some(app) = delete_parent.upgrade() {
                    app.set_status(error.into());
                }
                return;
            }
        };
        settings.current_profile = "IINA Default".into();
        match crate::key_binding_preferences::refresh(&window, &settings) {
            Ok(()) => window.invoke_settings_changed(),
            Err(error) => {
                if let Some(app) = delete_parent.upgrade() {
                    app.set_status(error.into());
                }
            }
        }
    });
    let reveal_window = window.as_weak();
    let reveal_parent = app.as_weak();
    window.on_key_binding_reveal_profile(move || {
        let Some(window) = reveal_window.upgrade() else {
            return;
        };
        let name = window.get_key_binding_current_profile().to_string();
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
        if let Err(error) = result
            && let Some(app) = reveal_parent.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let add_window = window.as_weak();
    window.on_key_binding_add_requested(move || {
        let Some(window) = add_window.upgrade() else {
            return;
        };
        if window.get_key_binding_profile_index() < 5 {
            return;
        }
        window.set_key_binding_editor_row(-1);
        window.set_key_binding_editor_key("".into());
        window.set_key_binding_editor_action("".into());
        window.set_key_binding_editor_comment("".into());
        window.set_key_binding_editor_kind(0);
        window.set_key_binding_editor_open(true);
    });
    let edit_window = window.as_weak();
    let edit_parent = app.as_weak();
    window.on_key_binding_edit_requested(move |row| {
        let Some(window) = edit_window.upgrade() else {
            return;
        };
        match crate::key_binding_preferences::mapping_for_row(&window, row) {
            Ok(mapping) => {
                window.set_key_binding_editor_row(row);
                window.set_key_binding_editor_key(mapping.key.into());
                window.set_key_binding_editor_action(mapping.action.into());
                window.set_key_binding_editor_comment(mapping.comment.unwrap_or_default().into());
                window.set_key_binding_editor_kind(
                    if mapping.kind == riina_core::key_bindings::ActionKind::Riina {
                        1
                    } else {
                        0
                    },
                );
                window.set_key_binding_editor_open(true);
            }
            Err(error) => {
                if let Some(app) = edit_parent.upgrade() {
                    app.set_status(error.into());
                }
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
            window.set_key_binding_editor_key(key.into());
        }
    });
    let remove_window = window.as_weak();
    let remove_parent = app.as_weak();
    window.on_key_binding_remove_requested(move |row| {
        let Some(window) = remove_window.upgrade() else {
            return;
        };
        match crate::key_binding_preferences::remove_mapping(&window, row) {
            Ok(()) => window.invoke_settings_changed(),
            Err(error) => {
                if let Some(app) = remove_parent.upgrade() {
                    app.set_status(error.into());
                }
            }
        }
    });
    let submit_window = window.as_weak();
    let submit_parent = app.as_weak();
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
                window.set_key_binding_editor_open(false);
                window.invoke_key_binding_settings_changed();
                window.invoke_settings_changed();
            }
            Err(error) => {
                if let Some(app) = submit_parent.upgrade() {
                    app.set_status(error.into());
                }
            }
        }
    });
    let cancel_window = window.as_weak();
    window.on_key_binding_editor_cancelled(move || {
        if let Some(window) = cancel_window.upgrade() {
            window.set_key_binding_editor_open(false);
        }
    });
}
