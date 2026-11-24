//! Scrollable, cascading menus inside the owning player surface.
use super::{Entry, Target};
use crate::PlayerWindow;
use riina_ui::{
    PlaylistPluginPopupState, PlaylistPopupBounds, PlaylistPopupInput, PlaylistPopupPanel,
    PlaylistPopupRow,
};
use slint::{ComponentHandle, LogicalPosition, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

const WIDTH: f32 = 224.0;
const PLAYER_MENU_WIDTH: f32 = 360.0;
const BACK_HEIGHT: f32 = 28.0;
const PADDING: f32 = 4.0;
const ITEM_HEIGHT: f32 = 24.0;
const SEPARATOR_HEIGHT: f32 = 8.0;

pub(super) fn dismiss(app: &PlayerWindow) {
    let popup = app.global::<PlaylistPluginPopupState>();
    if popup.get_shown() {
        popup.invoke_input(PlaylistPopupInput::Dismiss, 0, 0, 0.0);
    }
}

pub(super) fn show<A: 'static>(
    app: &PlayerWindow,
    point: LogicalPosition,
    entries: Vec<Entry<A>>,
    selected: impl FnOnce(A) + 'static,
) {
    show_client(app, point, entries, None, selected);
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn show_player_menu<A: 'static>(
    app: &PlayerWindow,
    point: LogicalPosition,
    family: i32,
    entries: Vec<Entry<A>>,
    selected: impl FnOnce(A) + 'static,
) {
    show_client(app, point, entries, Some(family), selected);
}

fn show_client<A: 'static>(
    app: &PlayerWindow,
    point: LogicalPosition,
    entries: Vec<Entry<A>>,
    player_menu_family: Option<i32>,
    selected: impl FnOnce(A) + 'static,
) {
    dismiss(app);
    let viewport = app.invoke_playlist_popup_bounds();
    if viewport.width <= PADDING * 2.0 || viewport.height <= PADDING * 2.0 {
        return;
    }
    let weak = app.as_weak();
    let menu = Menu::new(viewport, point, entries, player_menu_family.is_some());
    let popup = app.global::<PlaylistPluginPopupState>();
    popup.set_player_menu(player_menu_family.is_some());
    popup.set_player_menu_family(player_menu_family.unwrap_or(-1));
    popup.set_panels(ModelRc::from(menu.panels.clone()));
    let current = RefCell::new(Some((menu, selected)));
    popup.on_input(move |input, panel, row, anchor| {
        let Ok(mut current) = current.try_borrow_mut() else {
            // Updating a visible model can change which row is under the pointer.
            return;
        };
        let Some((menu, _)) = current.as_mut() else {
            return;
        };
        let result = menu.handle(input, panel, row, anchor);
        if result.is_some() || input == PlaylistPopupInput::Dismiss {
            let Some((_, selected)) = current.take() else {
                return;
            };
            drop(current);
            if let Some(app) = weak.upgrade() {
                let popup = app.global::<PlaylistPluginPopupState>();
                popup.set_shown(false);
                popup.set_panels(ModelRc::default());
                if let Some(action) = result {
                    selected(action);
                }
            }
        }
    });
    popup.set_shown(true);
}

enum Node<A> {
    Action(Option<A>),
    Branch(usize),
    Separator,
}

struct Group<A> {
    nodes: Vec<Node<A>>,
    rows: Rc<VecModel<PlaylistPopupRow>>,
    height: f32,
}

impl<A> Group<A> {
    fn store(entries: Vec<Entry<A>>, groups: &mut Vec<Self>) -> usize {
        let (nodes, rows): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .map(|entry| match entry {
                Entry::Separator => (
                    Node::Separator,
                    PlaylistPopupRow {
                        separator: true,
                        height: SEPARATOR_HEIGHT,
                        ..Default::default()
                    },
                ),
                Entry::Item {
                    title,
                    shortcut,
                    enabled,
                    checkable,
                    checked,
                    target,
                } => {
                    let node = match target {
                        Target::Action(action) => Node::Action(Some(action)),
                        Target::Submenu(entries) => Node::Branch(Self::store(entries, groups)),
                    };
                    let row = PlaylistPopupRow {
                        title: title.into(),
                        shortcut: shortcut.into(),
                        height: ITEM_HEIGHT,
                        enabled,
                        checkable,
                        checked,
                        submenu: matches!(node, Node::Branch(_)),
                        ..Default::default()
                    };
                    (node, row)
                }
            })
            .unzip();
        let index = groups.len();
        groups.push(Self {
            nodes,
            height: rows.iter().map(|row| row.height).sum(),
            rows: Rc::new(VecModel::from(rows)),
        });
        index
    }
}

struct Layer {
    group: usize,
    bounds: PlaylistPopupBounds,
    highlight: Option<usize>,
}

struct Menu<A> {
    viewport: PlaylistPopupBounds,
    panel_width: f32,
    player_menu: bool,
    groups: Vec<Group<A>>,
    layers: Vec<Layer>,
    panels: Rc<VecModel<PlaylistPopupPanel>>,
}

impl<A> Menu<A> {
    fn new(
        viewport: PlaylistPopupBounds,
        point: LogicalPosition,
        entries: Vec<Entry<A>>,
        player_menu: bool,
    ) -> Self {
        let mut groups = Vec::new();
        let root = Group::store(entries, &mut groups);
        let mut menu = Self {
            panel_width: if player_menu {
                PLAYER_MENU_WIDTH.min(viewport.width)
            } else {
                WIDTH.min(viewport.width)
            },
            player_menu,
            viewport,
            groups,
            layers: Vec::new(),
            panels: Rc::new(VecModel::default()),
        };
        let bounds = menu.bounds(root, point.x, point.y, false);
        menu.push(root, bounds, false);
        menu
    }

    fn bounds(&self, group: usize, x: f32, y: f32, back: bool) -> PlaylistPopupBounds {
        let viewport = &self.viewport;
        let width = self.panel_width;
        let heading = if back { BACK_HEIGHT } else { 0.0 };
        let height = (self.groups[group].height + 2.0 * PADDING + heading).min(viewport.height);
        PlaylistPopupBounds {
            x: x.clamp(viewport.x, viewport.x + (viewport.width - width)),
            y: y.clamp(viewport.y, viewport.y + (viewport.height - height)),
            width,
            height,
        }
    }

    fn submenu_x(&self, parent: &PlaylistPopupBounds) -> f32 {
        let viewport = &self.viewport;
        let width = self.panel_width;
        let right = parent.x + parent.width;
        if right + width <= viewport.x + viewport.width {
            right
        } else if parent.x - width >= viewport.x {
            parent.x - width
        } else {
            // Small windows show the new level over its parent. Left returns
            // to the parent menu. Pointer hover alone cannot enter this level.
            parent.x
        }
    }

    fn push(&mut self, group: usize, bounds: PlaylistPopupBounds, back: bool) {
        let state = &self.groups[group];
        self.panels.push(PlaylistPopupPanel {
            bounds: bounds.clone(),
            content_height: state.height,
            reveal: -1.0,
            reveal_serial: 0,
            back,
            rows: ModelRc::from(state.rows.clone()),
        });
        self.layers.push(Layer {
            group,
            bounds,
            highlight: None,
        });
    }

    fn trim(&mut self, count: usize) {
        while self.layers.len() > count {
            let last = self.layers.len() - 1;
            self.highlight(last, None, false);
            self.layers.pop();
            self.panels.remove(last);
        }
    }

    fn highlight(&mut self, panel: usize, row: Option<usize>, reveal: bool) {
        let layer = &mut self.layers[panel];
        let rows = &self.groups[layer.group].rows;
        if let Some(index) = layer.highlight
            && let Some(mut previous) = rows.row_data(index)
        {
            previous.highlighted = false;
            rows.set_row_data(index, previous);
        }
        layer.highlight = row;
        if let Some(index) = row
            && let Some(mut item) = rows.row_data(index)
        {
            item.highlighted = true;
            rows.set_row_data(index, item);
            if reveal && let Some(mut presentation) = self.panels.row_data(panel) {
                presentation.reveal = offset(rows, index);
                presentation.reveal_serial = presentation.reveal_serial.wrapping_add(1);
                self.panels.set_row_data(panel, presentation);
            }
        }
    }

    fn step(&mut self, forward: bool) {
        let panel = self.layers.len() - 1;
        let layer = &self.layers[panel];
        let rows = &self.groups[layer.group].rows;
        let count = rows.row_count();
        if count == 0 {
            return;
        }
        let starting = layer
            .highlight
            .unwrap_or(if forward { count - 1 } else { 0 });
        let selection = (1..=count)
            .map(|step| (starting + if forward { step } else { count - step }) % count)
            .find(|index| rows.row_data(*index).is_some_and(|row| row.enabled));
        self.highlight(panel, selection, true);
    }

    fn select_edge(&mut self, first: bool) {
        let panel = self.layers.len() - 1;
        let rows = &self.groups[self.layers[panel].group].rows;
        let count = rows.row_count();
        let selection = (0..count)
            .map(|index| if first { index } else { count - index - 1 })
            .find(|index| rows.row_data(*index).is_some_and(|row| row.enabled));
        self.highlight(panel, selection, true);
    }

    fn point(&mut self, panel: usize, row: usize, y: f32, choose: bool) -> Option<A> {
        let layer = self.layers.get(panel)?;
        let group = layer.group;
        let item = self.groups[group].rows.row_data(row)?;
        let x = self.submenu_x(&layer.bounds);
        let can_open_on_hover = x != layer.bounds.x;
        let branch = match self.groups[group].nodes.get(row)? {
            Node::Branch(child) if item.enabled => Some(*child),
            _ => None,
        };
        if let Some(child) = branch
            && self
                .layers
                .get(panel + 1)
                .is_some_and(|layer| layer.group == child)
        {
            return None;
        }
        self.trim(panel + 1);
        self.highlight(panel, item.enabled.then_some(row), false);
        if let Some(child) = branch
            && (choose || can_open_on_hover)
        {
            let back = self.player_menu && !can_open_on_hover;
            self.push(child, self.bounds(child, x, y, back), back);
        } else if item.enabled
            && choose
            && let Node::Action(action) = self.groups[group].nodes.get_mut(row)?
        {
            return action.take();
        }
        None
    }

    fn keyboard_activate(&mut self, enter: bool) -> Option<A> {
        let panel = self.layers.len() - 1;
        let layer = &self.layers[panel];
        let row = layer.highlight?;
        let group = &self.groups[layer.group];
        if enter || matches!(group.nodes.get(row), Some(Node::Branch(_))) {
            let item_y = offset(&group.rows, row);
            let y = layer.bounds.y + PADDING + item_y.min(layer.bounds.height - ITEM_HEIGHT);
            let result = self.point(panel, row, y, true);
            if self.layers.len() > panel + 1 {
                self.step(true);
            }
            return result;
        }
        None
    }

    fn handle(&mut self, input: PlaylistPopupInput, panel: i32, row: i32, y: f32) -> Option<A> {
        match input {
            PlaylistPopupInput::Hover | PlaylistPopupInput::Choose => self.point(
                usize::try_from(panel).ok()?,
                usize::try_from(row).ok()?,
                y,
                input == PlaylistPopupInput::Choose,
            ),
            PlaylistPopupInput::Next => {
                self.step(true);
                None
            }
            PlaylistPopupInput::Previous => {
                self.step(false);
                None
            }
            PlaylistPopupInput::First | PlaylistPopupInput::Last => {
                self.select_edge(input == PlaylistPopupInput::First);
                None
            }
            PlaylistPopupInput::Back => {
                self.trim(self.layers.len().saturating_sub(1).max(1));
                None
            }
            PlaylistPopupInput::Forward | PlaylistPopupInput::Enter => {
                self.keyboard_activate(input == PlaylistPopupInput::Enter)
            }
            PlaylistPopupInput::Dismiss => None,
        }
    }
}

fn offset(rows: &VecModel<PlaylistPopupRow>, index: usize) -> f32 {
    rows.iter().take(index).map(|row| row.height).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::{MenuShortcutRow, PlayerMenuActions};
    use slint::{
        LogicalSize, SharedString,
        platform::{Key, WindowEvent},
    };
    use std::cell::Cell;

    #[test]
    fn keyboard_crosses_nested_playlist_menus_and_skips_disabled_items()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        app.window().set_size(LogicalSize::new(390.0, 390.0));
        app.set_loaded(true);
        let mut shortcuts = vec![MenuShortcutRow::default(); 4];
        for (index, key) in [(2, "RightArrow"), (3, "LeftArrow")] {
            shortcuts[index].shortcut = slint::Keys::from_parts([key])?;
        }
        app.set_menu_shortcuts(ModelRc::new(VecModel::from(shortcuts)));
        let seeks = Rc::new(Cell::new(0));
        let recorded = seeks.clone();
        app.global::<PlayerMenuActions>()
            .on_seek_relative(move |_| recorded.set(recorded.get() + 1));
        app.show()?;
        slint::platform::update_timers_and_animations();
        let leaf = |title: &str, enabled, action| Entry::Item {
            title: title.into(),
            shortcut: String::new(),
            enabled,
            checkable: enabled,
            checked: enabled,
            target: Target::Action(action),
        };
        let entries = vec![
            leaf("Disabled", false, 0),
            Entry::Separator,
            Entry::Item {
                title: "Tools".into(),
                shortcut: String::new(),
                enabled: true,
                checkable: false,
                checked: false,
                target: Target::Submenu(vec![leaf("Select", true, 1)]),
            },
        ];
        let action = Rc::new(Cell::new(None));
        let selected = action.clone();
        show(
            &app,
            LogicalPosition::new(388.0, 383.0),
            entries,
            move |item| {
                selected.set(Some(item));
            },
        );
        let popup = app.global::<PlaylistPluginPopupState>();
        let key = |key: Key| {
            let text: SharedString = key.into();
            app.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            app.window()
                .dispatch_event(WindowEvent::KeyReleased { text });
            slint::platform::update_timers_and_animations();
        };
        slint::platform::update_timers_and_animations();
        assert!(popup.get_shown());
        let root = popup.get_panels().row_data(0).ok_or("Missing root menu")?;
        assert!(root.bounds.x + root.bounds.width <= 390.0);
        assert!(root.bounds.y + root.bounds.height <= 390.0);

        key(Key::DownArrow);
        key(Key::RightArrow);
        assert_eq!(popup.get_panels().row_count(), 2);
        key(Key::LeftArrow);
        assert_eq!(popup.get_panels().row_count(), 1);
        key(Key::RightArrow);
        key(Key::Return);
        assert_eq!(action.get(), Some(1));
        assert!(!popup.get_shown());
        assert_eq!(seeks.get(), 0);
        key(Key::RightArrow);
        assert_eq!(seeks.get(), 1);
        app.hide()?;
        Ok(())
    }
}
