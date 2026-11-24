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
    player: RealmMenu,
    global: RealmMenu,
}

#[derive(Clone)]
pub(super) struct Scope {
    directory: Directory,
    windows: Rc<RefCell<BTreeMap<u64, ToolWindow>>>,
    #[cfg(target_os = "macos")]
    closed: Option<Rc<crate::windows::close_observer::CloseObserver>>,
}

struct RealmMenu {
    scope: Scope,
    revision: u64,
    entries: Vec<(u64, String)>,
}

impl Scope {
    pub(super) fn new(directory: Directory) -> Self {
        Self {
            directory,
            windows: Rc::default(),
            #[cfg(target_os = "macos")]
            closed: None,
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn bind_application_close(
        &mut self,
        closed: Rc<crate::windows::close_observer::CloseObserver>,
    ) {
        self.closed = Some(closed);
    }

    fn open(&self, app: Option<&PlayerWindow>, id: u64) -> Result<(), String> {
        let realm = self
            .directory
            .get(id)
            .ok_or("The plugin entry has stopped")?;
        let mut windows = self.windows.borrow_mut();
        let window = match windows.entry(id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let window = ToolWindow::new(app, realm)?;
                #[cfg(target_os = "macos")]
                if let Some(closed) = self.closed.clone() {
                    window.window.window().on_close_requested(move || {
                        closed.notify();
                        slint::CloseRequestResponse::HideWindow
                    });
                }
                entry.insert(window)
            }
        };
        window.show()
    }

    #[cfg(target_os = "macos")]
    pub(super) fn open_windowless(&self, id: u64) -> Result<(), String> {
        self.open(None, id)
    }

    #[cfg(target_os = "macos")]
    pub(super) fn present_windowless(&self) -> bool {
        let Ok(mut windows) = self.windows.try_borrow_mut() else {
            return true;
        };
        let was_visible = visible(&windows);
        windows.retain(|id, _| self.directory.get(*id).is_some());
        let is_visible = visible(&windows);
        if was_visible
            && !is_visible
            && let Some(closed) = &self.closed
        {
            closed.notify();
        }
        refresh_visible(&mut windows);
        is_visible
    }

    #[cfg(target_os = "macos")]
    pub(super) fn is_visible(&self) -> bool {
        self.windows
            .try_borrow()
            .map_or(true, |windows| visible(&windows))
    }

    pub(super) fn close_all(&self) {
        self.windows.borrow_mut().clear();
    }
}

#[cfg(target_os = "macos")]
fn visible(windows: &BTreeMap<u64, ToolWindow>) -> bool {
    windows
        .values()
        .any(|window| window.window.window().is_visible())
}

fn refresh_visible(windows: &mut BTreeMap<u64, ToolWindow>) {
    for window in windows.values_mut() {
        if window.window.window().is_visible() {
            window.refresh();
        }
    }
}

impl RealmMenu {
    fn new(scope: Scope) -> Self {
        Self {
            scope,
            revision: 0,
            entries: Vec::new(),
        }
    }

    fn update(&mut self) -> bool {
        let Ok(mut windows) = self.scope.windows.try_borrow_mut() else {
            return false;
        };
        let changed = if let Some((revision, entries)) = self.scope.directory.list(self.revision) {
            self.revision = revision;
            windows.retain(|id, _| entries.iter().any(|(candidate, _)| id == candidate));
            self.entries = entries;
            true
        } else {
            false
        };
        refresh_visible(&mut windows);
        changed
    }

    fn rows(&self, prefix: &'static str) -> impl Iterator<Item = MenuCatalogRow> + '_ {
        self.entries.iter().map(move |(id, title)| MenuCatalogRow {
            token: format!("{prefix}:{id}").into(),
            label: title.as_str().into(),
            ..Default::default()
        })
    }
}

impl Presenter {
    pub(super) fn install(
        app: &PlayerWindow,
        entries: Directory,
        global: Scope,
    ) -> Rc<RefCell<Self>> {
        let presenter = Rc::new(RefCell::new(Self {
            player: RealmMenu::new(Scope::new(entries)),
            global: RealmMenu::new(global),
        }));
        let weak_presenter = Rc::downgrade(&presenter);
        let weak_app = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_open_plugin_developer_tool(move |token| {
                let Some((scope, id)) = token.split_once(':') else {
                    return;
                };
                if let (Ok(id), Some(presenter), Some(app)) =
                    (id.parse(), weak_presenter.upgrade(), weak_app.upgrade())
                    && let Ok(presenter) = presenter.try_borrow_mut()
                    && let Err(error) = presenter.open(&app, scope, id)
                {
                    crate::diagnostics::report(&app, error);
                }
            });
        presenter
    }

    pub(super) fn update(&mut self, app: &PlayerWindow) {
        let player_changed = self.player.update();
        let global_changed = self.global.update();
        if player_changed || global_changed {
            let rows = self
                .player
                .rows("player")
                .chain(self.global.rows("global"))
                .collect::<Vec<_>>();
            app.set_plugin_developer_rows(ModelRc::new(VecModel::from(rows)));
        }
    }

    pub(super) fn close_all(&mut self) {
        self.player.scope.close_all();
    }

    fn open(&self, app: &PlayerWindow, scope: &str, id: u64) -> Result<(), String> {
        match scope {
            "player" => self.player.scope.open(Some(app), id),
            "global" => self.global.scope.open(Some(app), id),
            _ => Err("Unknown plugin console scope".into()),
        }
    }
}

struct ToolWindow {
    window: PluginDeveloperTool,
    console: Rc<Console>,
    rows: Rc<VecModel<DeveloperConsoleRow>>,
    revision: u64,
}

impl ToolWindow {
    fn new(app: Option<&PlayerWindow>, realm: Realm) -> Result<Self, String> {
        let title = realm.title.clone();
        let window = PluginDeveloperTool::new().map_err(|error| error.to_string())?;
        let console = Rc::new(realm.attach().ok_or("The plugin console is unavailable")?);
        let rows = Rc::new(VecModel::default());
        window.set_lines(ModelRc::from(Rc::clone(&rows)));
        window.set_realm_title(title.into());
        window.set_primary_shortcut_is_meta(app.map_or(cfg!(target_os = "macos"), |app| {
            app.get_primary_shortcut_is_meta()
        }));
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
        if let Some(app) = app {
            platform::center_auxiliary_window(&window, app);
        }
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
