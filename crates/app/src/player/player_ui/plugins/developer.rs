//! One developer console window for each inspected player or global entry.
use crate::{
    PlayerWindow, platform,
    plugin_runtime::developer::{Command, Console, Directory, Kind, Line, Realm},
};
use riina_ui::{
    DeveloperConsoleMember, DeveloperConsoleRow, MenuCatalogRow, PlayerMenuActions,
    PluginDeveloperTool,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub(super) struct Presenter {
    entries: Directory,
    revision: u64,
    windows: BTreeMap<u64, ToolWindow>,
}

impl Presenter {
    pub(super) fn install(app: &PlayerWindow, entries: Directory) -> Rc<RefCell<Self>> {
        let presenter = Rc::new(RefCell::new(Self {
            entries,
            revision: 0,
            windows: BTreeMap::new(),
        }));
        let weak_presenter = Rc::downgrade(&presenter);
        let weak_app = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_open_plugin_developer_tool(move |token| {
                if let (Ok(id), Some(presenter), Some(app)) =
                    (token.parse(), weak_presenter.upgrade(), weak_app.upgrade())
                    && let Ok(mut presenter) = presenter.try_borrow_mut()
                    && let Err(error) = presenter.open(&app, id)
                {
                    crate::diagnostics::report(&app, error);
                }
            });
        presenter
    }

    pub(super) fn update(&mut self, app: &PlayerWindow) {
        if let Some((revision, entries)) = self.entries.list(self.revision) {
            self.revision = revision;
            self.windows
                .retain(|id, _| entries.iter().any(|(candidate, _)| id == candidate));
            let rows = entries
                .into_iter()
                .map(|(id, title)| MenuCatalogRow {
                    token: id.to_string().into(),
                    label: title.into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            app.set_plugin_developer_rows(ModelRc::new(VecModel::from(rows)));
        }
        for window in self.windows.values_mut() {
            if window.window.window().is_visible() {
                window.refresh();
            }
        }
    }

    pub(super) fn close_all(&mut self) {
        self.windows.clear();
    }

    fn open(&mut self, app: &PlayerWindow, id: u64) -> Result<(), String> {
        let realm = self.entries.get(id).ok_or("The plugin entry has stopped")?;
        let window = match self.windows.entry(id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(ToolWindow::new(app, realm)?)
            }
        };
        window.show()
    }
}

struct ToolWindow {
    window: PluginDeveloperTool,
    console: Rc<Console>,
    rows: Rc<VecModel<DeveloperConsoleRow>>,
    revision: u64,
}

impl ToolWindow {
    fn new(app: &PlayerWindow, realm: Realm) -> Result<Self, String> {
        let title = realm.title.clone();
        let window = PluginDeveloperTool::new().map_err(|error| error.to_string())?;
        let console = Rc::new(realm.attach().ok_or("The plugin console is unavailable")?);
        let rows = Rc::new(VecModel::default());
        window.set_lines(ModelRc::from(Rc::clone(&rows)));
        window.set_realm_title(title.into());
        window.set_primary_shortcut_is_meta(app.get_primary_shortcut_is_meta());
        let callback = Rc::downgrade(&console);
        let weak = window.as_weak();
        window.on_evaluate(move |source| {
            let source = source.trim();
            if source.is_empty() {
                return false;
            }
            if let (Some(console), Some(window)) = (callback.upgrade(), weak.upgrade()) {
                return submit(&window, &console, Command::Script(source.into()));
            }
            false
        });
        let callback = Rc::downgrade(&console);
        let weak = window.as_weak();
        window.on_show_globals(move || {
            if let (Some(console), Some(window)) = (callback.upgrade(), weak.upgrade()) {
                submit(&window, &console, Command::Globals);
            }
        });
        let callback = Rc::downgrade(&console);
        window.on_clear_history(move || {
            if let Some(console) = callback.upgrade() {
                console.clear();
            }
        });
        let callback = Rc::downgrade(&console);
        window.on_recall_command(move |offset| {
            callback
                .upgrade()
                .and_then(|console| console.previous_command(offset as usize))
                .unwrap_or_default()
                .into()
        });
        platform::center_auxiliary_window(&window, app);
        Ok(Self {
            window,
            console,
            rows,
            revision: 0,
        })
    }

    fn show(&mut self) -> Result<(), String> {
        self.refresh();
        platform::show_window(&self.window).map_err(|error| error.to_string())?;
        self.window
            .window()
            .with_winit_window(|window| window.focus_window());
        self.window.invoke_focus_editor();
        Ok(())
    }

    fn refresh(&mut self) {
        if let Some((revision, lines)) = self.console.lines(self.revision) {
            // Retain the row components that are still in the transcript so further
            // output does not close an expanded result or move keyboard focus.
            let retained = lines.partition_point(|line| line.sequence <= self.revision);
            if retained == 0 {
                self.rows.set_vec(Vec::new());
            } else {
                for _ in retained..self.rows.row_count() {
                    self.rows.remove(0);
                }
            }
            self.rows
                .extend(lines[retained..].iter().map(|line| console_row(line)));
            self.revision = revision;
            self.window.set_transcript_revision(revision as i32);
        }
    }
}

impl Drop for ToolWindow {
    fn drop(&mut self) {
        let _ = self.window.hide();
    }
}

fn submit(window: &PluginDeveloperTool, console: &Console, command: Command) -> bool {
    match console.submit(command) {
        Ok(()) => {
            window.set_status("".into());
            true
        }
        Err(error) => {
            window.set_status(error.into());
            false
        }
    }
}

fn console_row(line: &Line) -> DeveloperConsoleRow {
    use crate::diagnostics::Level;

    let (label, error) = match line.kind {
        Kind::Prompt => (
            format!("[{}]", line.prompt_number.unwrap_or(0)).into(),
            false,
        ),
        Kind::Result => ("â†’".into(), false),
        Kind::Error | Kind::Log(Level::Error) => ("Error".into(), true),
        Kind::Log(Level::Warning) => ("Warning".into(), false),
        Kind::Log(Level::Debug | Level::Info) => ("Log".into(), false),
    };
    let members = line
        .members
        .iter()
        .flatten()
        .map(|member| DeveloperConsoleMember {
            key: member.key.as_str().into(),
            value: member.value.as_str().into(),
        });
    DeveloperConsoleRow {
        label,
        text: line.text.as_str().into(),
        error,
        members: ModelRc::new(VecModel::from(members.collect::<Vec<_>>())),
    }
}
