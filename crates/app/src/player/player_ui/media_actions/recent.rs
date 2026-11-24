//! Separate resume history from the Open Recent document list.
use crate::{MenuCatalogRow, RecentMediaRow, WelcomeWindow};
use riina_core::{RecentMedia, time_label};
use std::path::{Path, PathBuf};

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

pub(crate) fn resumable(media: &[RecentMedia], enabled: bool) -> Option<&RecentMedia> {
    let last = enabled.then(|| media.first()).flatten()?;
    if let Some(file) = url::Url::parse(&last.path)
        .ok()
        .and_then(|url| url.to_file_path().ok())
    {
        return file.is_file().then_some(last);
    }
    Path::new(&last.path).is_file().then_some(last)
}

pub(crate) fn visible_documents<'a>(
    documents: &'a [RecentMedia],
    resume: Option<&RecentMedia>,
) -> impl Iterator<Item = &'a RecentMedia> {
    let resume_path = resume.map(|entry| entry.path.as_str());
    documents
        .iter()
        .filter(move |item| Some(item.path.as_str()) != resume_path)
}

pub(crate) fn refresh_welcome_history(
    welcome: &WelcomeWindow,
    media: &[RecentMedia],
    documents: &[RecentMedia],
    record_recent_files: bool,
    resume_last_position: bool,
) {
    let resume = resumable(media, record_recent_files && resume_last_position);
    let documents = if record_recent_files { documents } else { &[] };
    let document_rows = visible_documents(documents, resume)
        .cloned()
        .collect::<Vec<_>>();
    welcome.set_recent_media(slint::ModelRc::new(slint::VecModel::from(
        resume
            .map(|item| recent_media_rows(std::slice::from_ref(item)))
            .unwrap_or_default(),
    )));
    welcome.set_recent_documents(slint::ModelRc::new(slint::VecModel::from(
        recent_media_rows(&document_rows),
    )));
    welcome.set_selection_index(if resume.is_some() || document_rows.is_empty() {
        -1
    } else {
        0
    });
}
