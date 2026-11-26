//! Separate resume history from the Open Recent document list.
use crate::{MenuCatalogRow, RecentMediaRow, WelcomeWindow, file_actions::local_media_path};
use riina_core::{RecentMedia, osc_preferences::ControllerOptions};
use slint::Model;
use std::path::{Path, PathBuf};

fn welcome_row(item: &RecentMedia, resume: bool) -> RecentMediaRow {
    let path = local_media_path(&item.path);
    let title = path.as_deref().and_then(Path::file_name).map_or_else(
        || recent_title(item),
        |name| name.to_string_lossy().into_owned(),
    );
    RecentMediaRow {
        token: item.path.as_str().into(),
        title: title.into(),
        position_label: if resume {
            ControllerOptions::default()
                .elapsed_label(Some(item.position))
                .into()
        } else {
            Default::default()
        },
    }
}

pub(crate) fn recent_open_menu_rows(media: &[riina_core::RecentMedia]) -> Vec<MenuCatalogRow> {
    media
        .iter()
        .map(|item| MenuCatalogRow {
            token: item.path.as_str().into(),
            label: recent_title(item).into(),
            selected: false,
        })
        .collect()
}

fn recent_title(item: &RecentMedia) -> String {
    if item.title.trim().is_empty() {
        Path::new(&item.path).file_name().map_or_else(
            || item.path.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
    } else {
        item.title.clone()
    }
}

pub(crate) fn resumable(media: &[RecentMedia], enabled: bool) -> Option<&RecentMedia> {
    let last = enabled.then(|| media.first()).flatten()?;
    local_media_path(&last.path)
        .filter(|path| path.is_file())
        .map(|_| last)
}

pub(crate) fn visible_documents<'a>(
    documents: &'a [RecentMedia],
    resume: Option<&RecentMedia>,
) -> impl Iterator<Item = &'a RecentMedia> {
    let canonical_resume = resume.and_then(|entry| canonical_path(&entry.path));
    documents.iter().filter(move |item| {
        resume.is_none_or(|entry| entry.path != item.path)
            && canonical_resume
                .as_ref()
                .is_none_or(|path| canonical_path(&item.path).as_ref() != Some(path))
    })
}

fn canonical_path(reference: &str) -> Option<PathBuf> {
    local_media_path(reference)?.canonicalize().ok()
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
    let current = welcome.get_recent_documents();
    let selected = usize::try_from(welcome.get_selection_index())
        .ok()
        .and_then(|index| current.row_data(index));
    let document_rows = visible_documents(documents, resume)
        .map(|entry| welcome_row(entry, false))
        .collect::<Vec<_>>();
    let fallback = if resume.is_some() || document_rows.is_empty() {
        -1
    } else {
        0
    };
    let selection = selected
        .and_then(|selected| {
            document_rows
                .iter()
                .position(|row| row.token == selected.token)
        })
        .and_then(|index| i32::try_from(index).ok())
        .unwrap_or(fallback);
    let resume_rows = resume
        .map(|entry| welcome_row(entry, true))
        .into_iter()
        .collect();
    crate::ui_models::set_rows_if_changed(&welcome.get_recent_media(), resume_rows, |rows| {
        welcome.set_recent_media(rows)
    });
    crate::ui_models::set_rows_if_changed(&current, document_rows, |rows| {
        welcome.set_recent_documents(rows)
    });
    welcome.set_selection_index(selection);
}
