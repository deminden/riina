//! Separate resume history from the Open Recent document list.
use crate::{MenuCatalogRow, PlayerWindow, RecentMediaRow, WelcomeWindow, platform};
use riina_core::{Preferences, time_label};
use slint::ComponentHandle;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub(crate) fn recent_media_rows(media: &[riina_core::RecentMedia]) -> Vec<RecentMediaRow> {
    media
        .iter()
        .map(|item| {
            let title = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            RecentMediaRow {
                title: title.into(),
                position_label: time_label(Some(item.position)).into(),
            }
        })
        .collect()
}

pub(crate) fn recent_open_menu_rows(media: &[riina_core::RecentMedia]) -> Vec<MenuCatalogRow> {
    media
        .iter()
        .map(|item| {
            let label = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            MenuCatalogRow {
                token: item.path.clone().into(),
                label: label.into(),
                selected: false,
            }
        })
        .collect()
}

pub(super) fn remember_recent_open_document(
    app: &PlayerWindow,
    documents: &Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    preferences_path: &std::path::Path,
    path: String,
    title: String,
) {
    if !app
        .global::<riina_ui::PlayerPreferences>()
        .get_record_recent_files()
        || path.is_empty()
    {
        return;
    }
    let native_path = path.clone();
    let mut documents = documents.borrow_mut();
    documents.retain(|document| document.path != path);
    documents.insert(
        0,
        riina_core::RecentMedia {
            path,
            title,
            position: 0.0,
        },
    );
    documents.truncate(10);
    let recent = documents.clone();
    drop(documents);
    app.global::<riina_ui::PlayerCatalogState>()
        .set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
            recent_open_menu_rows(&recent),
        )));
    app.global::<riina_ui::PlayerMenuActions>()
        .invoke_video_menu_changed();
    if let Err(error) = persist_recent_open_documents(preferences_path, &recent) {
        app.set_status(format!("Could not save Open Recent: {error}").into());
    }
    if let Err(error) = platform::remember_recent_document(&native_path) {
        app.set_status(format!("Could not update the system recent documents: {error}").into());
    }
}

pub(super) fn persist_recent_open_documents(
    preferences_path: &std::path::Path,
    documents: &[riina_core::RecentMedia],
) -> std::io::Result<()> {
    crate::preference_store::update_automatically(
        preferences_path,
        &Preferences::default(),
        |preferences| {
            preferences.recent_open_documents = documents.to_vec();
            Ok(())
        },
    )
    .map_err(std::io::Error::other)
}

pub(crate) fn resume_media_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    if resume_last_position {
        recent_media_rows(media.get(..1).unwrap_or_default())
    } else {
        Vec::new()
    }
}

pub(crate) fn recent_document_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    // IINA removes the last-played item from the document table only while its
    // separate Resume action is enabled.
    let offset = if resume_last_position { 1 } else { 0 };
    recent_media_rows(media.get(offset..).unwrap_or_default())
}

pub(crate) fn refresh_welcome_history(
    welcome: &WelcomeWindow,
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) {
    welcome.set_recent_media(slint::ModelRc::new(slint::VecModel::from(
        resume_media_rows(media, resume_last_position),
    )));
    welcome.set_recent_documents(slint::ModelRc::new(slint::VecModel::from(
        recent_document_rows(media, resume_last_position),
    )));
    welcome.set_recent_document_offset(if resume_last_position { 1 } else { 0 });
    welcome.set_selection_index(if resume_last_position || media.is_empty() {
        -1
    } else {
        0
    });
}
