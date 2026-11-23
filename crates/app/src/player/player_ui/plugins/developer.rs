//! One developer console window for each inspected player or global entry.
use crate::{
    PlayerWindow, platform,
    plugin_runtime::developer::{Command, Console, Directory, Kind, Line, Realm},
};
use riina_ui::{DeveloperConsoleRow, MenuCatalogRow, PlayerMenuActions, PluginDeveloperTool};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
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
    revision: u64,
}

impl ToolWindow {
    fn new(app: &PlayerWindow, realm: Realm) -> Result<Self, String> {
        let title = realm.title.clone();
        let window = PluginDeveloperTool::new().map_err(|error| error.to_string())?;
        let console = Rc::new(realm.attach().ok_or("The plugin console is unavailable")?);
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
        platform::center_auxiliary_window(&window, app);
        Ok(Self {
            window,
            console,
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
            self.revision = revision;
            self.window.set_lines(ModelRc::new(VecModel::from(
                lines.into_iter().map(console_row).collect::<Vec<_>>(),
            )));
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

fn console_row(line: Line) -> DeveloperConsoleRow {
    use crate::diagnostics::Level;

    let (label, error) = match line.kind {
        Kind::Prompt => ("In", false),
        Kind::Result => ("Out", false),
        Kind::Error | Kind::Log(Level::Error) => ("Error", true),
        Kind::Log(Level::Warning) => ("Warning", false),
        Kind::Log(Level::Debug | Level::Info) => ("Log", false),
    };
    DeveloperConsoleRow {
        label: label.into(),
        text: line.text.into(),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_backend_testing::ElementHandle;
    use slint::LogicalSize;

    #[test]
    fn developer_tool_keeps_the_run_toolbar_compact() -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = PluginDeveloperTool::new()?;
        window.window().set_size(LogicalSize::new(500.0, 400.0));
        window.show()?;
        slint::platform::update_timers_and_animations();
        let run = ElementHandle::find_by_accessible_label(&window, "Run")
            .next()
            .ok_or("Missing Run button")?;
        assert!(
            run.size().height <= 36.0,
            "Run button size: {:?}",
            run.size()
        );
        assert!(
            run.absolute_position().y >= 240.0,
            "Run button position: {:?}",
            run.absolute_position()
        );
        window.hide()?;
        Ok(())
    }
}
