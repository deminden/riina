//! Chapter selections belong to the observed media and chapter time.
use super::PlayerContext;
use crate::{CatalogRow, MenuCatalogRow, PlayerWindow};
use riina_core::{Command, Playback, osc_preferences::ControllerOptions, sync::lock};
use riina_ui::{PlaybackTimelineState, PlayerCatalogState, PlayerSidebarState};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::sync::Arc;

pub(super) fn activate(ctx: &PlayerContext, token: &str, notify: bool) {
    let Some((generation, index, start)) = parse_token(token) else {
        ctx.report("Invalid chapter selection".to_owned());
        return;
    };
    let Some(app) = ctx.app() else { return };
    if !app.get_loaded() || lock(ctx.playback()).generation != generation {
        return;
    }
    ctx.activity();
    let command = Command::ActivateChapter {
        index,
        start,
        notify,
    };
    if let Err(error) = ctx.commands().send_for_media(generation, command) {
        ctx.report(error);
    }
}

fn parse_token(token: &str) -> Option<(u64, usize, f64)> {
    let (generation, chapter) = token.split_once(':')?;
    let (index, bits) = chapter.split_once(':')?;
    Some((
        generation.parse().ok()?,
        index.parse().ok()?,
        f64::from_bits(u64::from_str_radix(bits, 16).ok()?),
    ))
}

pub(super) fn update(app: &PlayerWindow, previous: Option<&Playback>, state: &Playback) {
    let catalog = app.global::<PlayerCatalogState>();
    let Some(old) = previous.filter(|old| old.generation == state.generation) else {
        let sidebar = app.global::<PlayerSidebarState>();
        sidebar.set_chapter_cursor(-1);
        sidebar.set_chapters_scroll_y(0.0);
        replace_rows(app, &catalog, state);
        return;
    };
    if !Arc::ptr_eq(&old.chapters, &state.chapters) && old.chapters != state.chapters {
        replace_rows(app, &catalog, state);
        return;
    }
    if state.chapters.is_empty() {
        return;
    }
    let (previous_active, active) = (old.current_chapter_index(), state.current_chapter_index());
    if previous_active != active {
        update_sidebar_marker(&catalog.get_chapters(), previous_active, false);
        update_sidebar_marker(&catalog.get_chapters(), active, true);
    }
    let menu_index = state.chapter_menu_index();
    if old.chapter_menu_index() != menu_index {
        let current = catalog.get_chapter_menu_rows();
        let menu = current.iter().enumerate().map(|(index, mut row)| {
            row.selected = menu_index == Some(index);
            row
        });
        catalog.set_chapter_menu_rows(ModelRc::new(VecModel::from(menu.collect::<Vec<_>>())));
    }
}

fn update_sidebar_marker(rows: &ModelRc<CatalogRow>, index: Option<usize>, selected: bool) {
    if let Some(index) = index
        && let Some(mut row) = rows.row_data(index)
    {
        row.selected = selected;
        rows.set_row_data(index, row);
    }
}

fn replace_rows(app: &PlayerWindow, catalog: &PlayerCatalogState<'_>, state: &Playback) {
    let clock = ControllerOptions::default();
    let active = state.current_chapter_index();
    let menu_index = state.chapter_menu_index();
    let starts: Vec<_> = state
        .chapters
        .iter()
        .map(|chapter| clock.elapsed_label(Some(chapter.time)))
        .collect();
    let template = starts.last().map_or("", String::as_str);
    let mut rows = Vec::with_capacity(state.chapters.len());
    let mut menu = Vec::with_capacity(state.chapters.len());
    for (index, chapter) in state.chapters.iter().enumerate() {
        let token = SharedString::from(format!(
            "{}:{index}:{:016x}",
            state.generation,
            chapter.time.to_bits()
        ));
        let title = chapter.display_title(index);
        let start = &starts[index];
        let end = starts.get(index + 1).map_or("End", String::as_str);
        // Use the final timestamp's leading zero and colon pattern to line up shorter entries.
        let padding: String = template
            .chars()
            .take(template.len().saturating_sub(start.len()))
            .map(|character| if character == ':' { ':' } else { '0' })
            .collect();
        menu.push(MenuCatalogRow {
            token: token.clone(),
            label: format!(
                "{padding}{start} – {}",
                chapter.title.as_deref().unwrap_or(title.as_ref())
            )
            .into(),
            selected: menu_index == Some(index),
        });
        rows.push(CatalogRow {
            token,
            label: title.as_ref().into(),
            detail: format!("{start} → {end}").into(),
            selected: active == Some(index),
            ..CatalogRow::default()
        });
    }
    crate::ui_models::set_rows_if_changed(&catalog.get_chapters(), rows, |rows| {
        catalog.set_chapters(rows)
    });
    crate::ui_models::set_rows_if_changed(&catalog.get_chapter_menu_rows(), menu, |rows| {
        catalog.set_chapter_menu_rows(rows)
    });
    // The reference omits the first boundary because it coincides with the bar origin.
    let timeline = app.global::<PlaybackTimelineState>();
    let times = state
        .chapters
        .iter()
        .skip(1)
        .map(|chapter| chapter.time as f32)
        .filter(|time| time.is_finite() && *time >= 0.0)
        .collect();
    crate::ui_models::set_rows_if_changed(&timeline.get_chapter_times(), times, |times| {
        timeline.set_chapter_times(times)
    });
}
