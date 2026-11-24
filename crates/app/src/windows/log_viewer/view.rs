use super::{LogViewerWindow, reader::LogFiles, selection::Selection};
use riina_ui::{LogNavigation, LogSubsystemChoice, LogViewerRow};
use slint::{Model, ModelRc, VecModel};
use std::{collections::BTreeSet, fmt::Write as _, path::Path, rc::Rc};

pub(super) struct View {
    logs: LogFiles,
    last_error: Option<String>,
    filters: BTreeSet<String>,
    subsystem_rows: Rc<VecModel<LogSubsystemChoice>>,
    ids: Vec<u64>,
    rows: Rc<VecModel<LogViewerRow>>,
    selection: Selection,
}

impl View {
    pub(super) fn new(window: &LogViewerWindow, preferences: &Path) -> Self {
        let subsystem_rows = Rc::new(VecModel::default());
        window.set_subsystems(ModelRc::from(subsystem_rows.clone()));
        Self {
            logs: LogFiles::new(preferences),
            last_error: None,
            filters: BTreeSet::new(),
            subsystem_rows,
            ids: Vec::new(),
            rows: Rc::new(VecModel::default()),
            selection: Selection::default(),
        }
    }

    pub(super) fn refresh(&mut self, window: &LogViewerWindow, force: bool) {
        let (changed, error) = self.logs.refresh();
        if !force && !changed && error == self.last_error {
            return;
        }
        self.last_error = error;
        self.render(window);
    }

    pub(super) fn toggle_subsystem(&mut self, window: &LogViewerWindow, name: &str) {
        if !self.filters.remove(name) {
            self.filters.insert(name.to_owned());
        }
        self.render(window);
    }

    pub(super) fn clear_subsystems(&mut self, window: &LogViewerWindow) {
        self.filters.clear();
        self.render(window);
    }

    pub(super) fn render(&mut self, window: &LogViewerWindow) {
        let entries = self.logs.entries();
        let subsystem_names = entries
            .iter()
            .map(|entry| entry.subsystem.as_str())
            .filter(|name| !name.is_empty())
            .chain(self.filters.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        self.update_subsystems(window, subsystem_names);

        let total = entries.len();
        let predicate = Filter::new(window, &self.filters);
        let visible: Vec<_> = entries
            .into_iter()
            .filter(|entry| predicate.matches(entry))
            .collect();
        let ids: Vec<_> = visible.iter().map(|entry| entry.id).collect();
        if ids != self.ids {
            self.selection.retain_visible(&ids);
            let rows: Vec<_> = visible
                .iter()
                .map(|entry| LogViewerRow {
                    level: level_label(&entry.level).into(),
                    timestamp: entry.timestamp.as_str().into(),
                    subsystem: entry.subsystem.as_str().into(),
                    message: entry.message.as_str().into(),
                    level_color: level_color(entry.rank),
                    selected: self.selection.contains(entry.id),
                })
                .collect();
            self.ids = ids;
            self.rows = Rc::new(VecModel::from(rows));
            window.set_rows(ModelRc::from(self.rows.clone()));
            self.project_selection(window);
            window.invoke_logs_changed();
        }
        let status = if let Some(error) = &self.last_error {
            error.clone()
        } else if !self.logs.available() {
            "No log files are available".into()
        } else {
            format!("{} of {total} log entries", self.ids.len())
        };
        window.set_status_text(status.into());
    }

    fn update_subsystems(&self, window: &LogViewerWindow, names: BTreeSet<&str>) {
        let options: Vec<_> = names
            .into_iter()
            .map(|name| LogSubsystemChoice {
                name: name.into(),
                selected: self.filters.contains(name),
            })
            .collect();
        let current = &self.subsystem_rows;
        if current.row_count() != options.len()
            || current
                .iter()
                .zip(&options)
                .any(|(row, choice)| row.name != choice.name)
        {
            current.set_vec(options);
        } else {
            for (index, (row, choice)) in current.iter().zip(options).enumerate() {
                if row.selected != choice.selected {
                    current.set_row_data(index, choice);
                }
            }
        }
        window.set_selected_subsystem_count(i32::try_from(self.filters.len()).unwrap_or(i32::MAX));
    }

    pub(super) fn select(
        &mut self,
        window: &LogViewerWindow,
        index: i32,
        toggle: bool,
        range: bool,
    ) {
        if let Ok(index) = usize::try_from(index) {
            self.selection.select(&self.ids, index, toggle, range);
            self.project_selection(window);
        }
    }

    pub(super) fn select_all(&mut self, window: &LogViewerWindow) {
        self.selection.select_all(&self.ids);
        self.project_selection(window);
    }

    pub(super) fn navigate(
        &mut self,
        window: &LogViewerWindow,
        action: LogNavigation,
        range: bool,
    ) {
        let Some(last) = self.ids.len().checked_sub(1) else {
            return;
        };
        let current = self.selection.active_index(&self.ids);
        let index = match action {
            LogNavigation::First => 0,
            LogNavigation::Last => last,
            LogNavigation::Previous => current.map_or(last, |index| index.saturating_sub(1)),
            LogNavigation::Next => current.map_or(0, |index| (index + 1).min(last)),
        };
        self.selection.select(&self.ids, index, false, range);
        self.project_selection(window);
        if let Ok(index) = i32::try_from(index) {
            window.invoke_reveal_row(index);
        }
    }

    fn project_selection(&self, window: &LogViewerWindow) {
        let mut any = false;
        for (index, (id, mut row)) in self.ids.iter().zip(self.rows.iter()).enumerate() {
            let selected = self.selection.contains(*id);
            any |= selected;
            if row.selected != selected {
                row.selected = selected;
                self.rows.set_row_data(index, row);
            }
        }
        window.set_has_selection(any);
    }

    pub(super) fn copy(&self, clicked: i32) -> String {
        let target = usize::try_from(clicked)
            .ok()
            .and_then(|index| self.ids.get(index));
        self.snapshot(|entry| {
            target
                .filter(|id| !self.selection.contains(**id))
                .map_or_else(|| self.selection.contains(entry.id), |id| *id == entry.id)
        })
    }

    pub(super) fn export(&self, window: &LogViewerWindow, filtered: bool) -> String {
        let filter = Filter::new(window, &self.filters);
        self.snapshot(|entry| !filtered || filter.matches(entry))
    }

    fn snapshot(&self, keep: impl Fn(&super::reader::Entry) -> bool) -> String {
        let mut text = String::new();
        for entry in self.logs.entries().into_iter().filter(|entry| keep(entry)) {
            let _ = writeln!(text, "{}", entry.raw);
        }
        text
    }
}

struct Filter<'a> {
    query: String,
    maximum_rank: u8,
    subsystems: &'a BTreeSet<String>,
}

impl<'a> Filter<'a> {
    fn new(window: &LogViewerWindow, subsystems: &'a BTreeSet<String>) -> Self {
        Self {
            query: window.get_search_text().to_lowercase(),
            maximum_rank: u8::try_from(window.get_level_index() - 1).unwrap_or(u8::MAX),
            subsystems,
        }
    }

    fn matches(&self, entry: &super::reader::Entry) -> bool {
        entry.rank <= self.maximum_rank
            && (self.subsystems.is_empty() || self.subsystems.contains(&entry.subsystem))
            && (self.query.is_empty() || entry.message.to_lowercase().contains(&self.query))
    }
}

fn level_label(level: &str) -> &str {
    match level {
        "warn" => "warning",
        "v" => "verbose",
        "vv" => "verbose 2",
        "vvv" => "verbose 3",
        "vvvv" => "verbose 4",
        "vvvvv" => "verbose 5",
        "vvvvvv" => "verbose 6",
        other => other,
    }
}

fn level_color(rank: u8) -> slint::Color {
    match rank {
        0 | 1 => slint::Color::from_rgb_u8(190, 44, 46),
        2 => slint::Color::from_rgb_u8(182, 112, 0),
        3 => slint::Color::from_rgb_u8(50, 106, 70),
        _ => slint::Color::from_rgb_u8(96, 96, 105),
    }
}
