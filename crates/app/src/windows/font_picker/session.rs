//! Keep one chooser and route its selection to the window that opened it.
use super::{discover_catalog, refresh_font};
use crate::{FontPicker, PlayerWindow, PreferencesWindow, platform};
use riina_core::{Command, subtitle_controls::font_from_text};
use riina_mpv::CommandSender;
use riina_ui::{
    PlaybackSubtitleState, PlayerAppearance, PlayerControlActions, PlayerDialogState,
    PlayerMenuActions,
};
use slint::{
    ComponentHandle, SharedString, Weak,
    winit_030::{WinitWindowAccessor, winit::window::Theme},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Default)]
pub(crate) struct ApplicationFontPicker {
    picker: RefCell<Option<FontPicker>>,
    open: RefCell<Option<Open>>,
    appearance: slint::Timer,
}

#[derive(Clone)]
enum Target {
    Player(Weak<PlayerWindow>, CommandSender),
    Settings(Weak<PreferencesWindow>),
}

struct Open {
    owner: Rc<()>,
    target: Target,
    #[cfg(any(windows, target_os = "macos"))]
    modal_request: Rc<()>,
    #[cfg(any(windows, target_os = "macos"))]
    modal: Option<platform::font_picker_window::Modal>,
}

impl ApplicationFontPicker {
    pub(crate) fn connect_player(self: &Rc<Self>, window: &PlayerWindow, engine: &CommandSender) {
        let owner = Rc::new(());
        let service = self.clone();
        let closing = owner.clone();
        window.on_dismiss_font_picker(move || service.dismiss(&closing));
        let target = Target::Player(window.as_weak(), engine.clone());
        let service = self.clone();
        window
            .global::<PlayerMenuActions>()
            .on_choose_subtitle_font(move || {
                let Target::Player(window, _) = &target else {
                    return;
                };
                let Some(window) = window.upgrade() else {
                    return;
                };
                if window.get_loaded() && !window.global::<PlayerDialogState>().get_dialog_open() {
                    let font = window.global::<PlaybackSubtitleState>().get_subtitle_font();
                    service.present(&owner, &target, font);
                }
            });
    }

    pub(crate) fn connect_settings(self: &Rc<Self>, window: &PreferencesWindow) {
        let owner = Rc::new(());
        let service = self.clone();
        let closing = owner.clone();
        window.on_close_profile_font_picker(move || service.dismiss(&closing));
        let service = self.clone();
        let raising = owner.clone();
        window.on_raise_font_picker(move || {
            if service.owns(&raising) {
                service.raise();
            }
        });
        let target = Target::Settings(window.as_weak());
        let service = self.clone();
        window.on_choose_subtitle_font(move || {
            let Target::Settings(window) = &target else {
                return;
            };
            if let Some(window) = window.upgrade() {
                let font = window
                    .global::<crate::SubtitleSettingsState>()
                    .get_subtitle_font();
                service.present(&owner, &target, font);
            }
        });
    }

    fn present(self: &Rc<Self>, owner: &Rc<()>, target: &Target, font: SharedString) {
        let sheet_open = self
            .open
            .borrow()
            .as_ref()
            .is_some_and(|open| matches!(open.target, Target::Settings(_)));
        if sheet_open {
            self.raise();
            return;
        }
        let Ok(picker) = self
            .picker(target)
            .inspect_err(|error| target.report(error.to_string()))
        else {
            return;
        };
        if !self.owns(owner) {
            self.finish(None);
            target.center(&picker);
        }
        refresh_font(&picker, font);
        if let Some(dark) = target.dark() {
            set_theme(&picker, dark);
        }
        if let Err(error) = platform::show_window(&picker) {
            self.finish(None);
            target.report(error.to_string());
            return;
        }
        picker.set_picker_active(true);
        platform::enable_font_picker_acrylic(&picker);
        self.open.replace(Some(Open {
            owner: owner.clone(),
            target: target.clone(),
            #[cfg(any(windows, target_os = "macos"))]
            modal_request: Rc::new(()),
            #[cfg(any(windows, target_os = "macos"))]
            modal: None,
        }));
        if let Target::Settings(owner) = target
            && let Some(window) = owner.upgrade()
        {
            window.set_font_picker_open(true);
            window.set_font_picker_input_shield(true);
            #[cfg(any(windows, target_os = "macos"))]
            self.attach_modal(owner.clone(), &picker);
        }
        self.watch_theme(target, &picker);
        self.raise();
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn attach_modal(self: &Rc<Self>, owner: Weak<PreferencesWindow>, picker: &FontPicker) {
        let Some(request) = self
            .open
            .borrow()
            .as_ref()
            .map(|open| Rc::downgrade(&open.modal_request))
        else {
            return;
        };
        let service = Rc::downgrade(self);
        let picker = picker.as_weak();
        // A freshly shown Slint window can still be awaiting Winit realization.
        let _ = slint::spawn_local(async move {
            let (Some(owner), Some(picker)) = (owner.upgrade(), picker.upgrade()) else {
                return;
            };
            let Ok(native_owner) = owner.window().winit_window().await else {
                return;
            };
            let Ok(native_picker) = picker.window().winit_window().await else {
                return;
            };
            let (Some(service), Some(request)) = (service.upgrade(), request.upgrade()) else {
                return;
            };
            let current = |open: &Open| Rc::ptr_eq(&open.modal_request, &request);
            if !service.open.borrow().as_ref().is_some_and(current) {
                return;
            }
            let modal = platform::font_picker_window::attach(&native_owner, &native_picker);
            match modal {
                Ok(modal) => {
                    let mut state = service.open.borrow_mut();
                    if let Some(open) = state.as_mut().filter(|open| current(open)) {
                        open.modal = Some(modal);
                        drop(state);
                        owner.set_font_picker_input_shield(false);
                        native_picker.focus_window();
                    }
                }
                Err(error) => {
                    service.finish(None);
                    owner.set_profile_status(error.into());
                }
            }
        });
    }

    fn picker(self: &Rc<Self>, target: &Target) -> Result<FontPicker, slint::PlatformError> {
        if let Some(picker) = self.picker.borrow().as_ref() {
            return Ok(picker.clone_strong());
        }
        let picker = FontPicker::new()?;
        crate::appearance_preferences::install_system_typography(&picker);
        picker.set_picker_active(false);
        let service = Rc::downgrade(self);
        picker.on_accept_font(move |font| {
            if let Some(service) = service.upgrade() {
                service.finish(Some(font));
            }
        });
        let service = Rc::downgrade(self);
        picker.on_cancel(move || {
            if let Some(service) = service.upgrade() {
                service.finish(None);
            }
        });
        let service = Rc::downgrade(self);
        picker.window().on_close_requested(move || {
            if let Some(service) = service.upgrade() {
                service.finish(None);
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
        discover_catalog(picker.as_weak(), |error| target.report(error));
        self.picker.replace(Some(picker.clone_strong()));
        Ok(picker)
    }

    fn owns(&self, owner: &Rc<()>) -> bool {
        self.open
            .borrow()
            .as_ref()
            .is_some_and(|open| Rc::ptr_eq(&open.owner, owner))
    }

    fn dismiss(&self, owner: &Rc<()>) {
        if self.owns(owner) {
            self.finish(None);
        }
    }

    fn finish(&self, selection: Option<SharedString>) {
        let Some(open) = self.open.take() else { return };
        self.appearance.stop();
        #[cfg(any(windows, target_os = "macos"))]
        drop(open.modal);
        if let Some(picker) = self.picker.borrow().as_ref() {
            hide(picker);
        }
        let target = open.target;
        target.finished();
        if let Some(selection) = selection
            && let Err(error) = font_name(selection).and_then(|font| target.apply(font))
        {
            target.report(error);
        }
    }

    fn raise(&self) {
        if let Some(picker) = self.picker.borrow().as_ref() {
            picker
                .window()
                .with_winit_window(|window| window.focus_window());
        }
    }

    fn watch_theme(self: &Rc<Self>, target: &Target, picker: &FontPicker) {
        let target = target.clone();
        let picker = picker.as_weak();
        let service = Rc::downgrade(self);
        self.appearance.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                if let (Some(picker), Some(dark)) = (picker.upgrade(), target.dark()) {
                    if picker.get_dark_theme() != dark {
                        set_theme(&picker, dark);
                    }
                } else if let Some(service) = service.upgrade() {
                    service.finish(None);
                }
            },
        );
    }

    #[cfg(test)]
    fn visible_picker(&self) -> Option<FontPicker> {
        self.picker
            .borrow()
            .as_ref()
            .filter(|picker| picker.get_picker_active())
            .map(ComponentHandle::clone_strong)
    }
}

impl Target {
    fn dark(&self) -> Option<bool> {
        match self {
            Self::Player(window, _) => window
                .upgrade()
                .map(|window| window.global::<PlayerAppearance>().get_dark_theme()),
            Self::Settings(window) => window.upgrade().map(|_| false),
        }
    }

    fn center(&self, picker: &FontPicker) {
        match self {
            Self::Player(window, _) => {
                if let Some(window) = window.upgrade() {
                    platform::center_auxiliary_window(picker, &window);
                }
            }
            Self::Settings(window) => {
                if let Some(window) = window.upgrade() {
                    platform::center_auxiliary_window(picker, &window);
                }
            }
        }
    }

    fn finished(&self) {
        match self {
            Self::Player(window, _) => {
                if let Some(window) = window.upgrade() {
                    window.global::<PlayerControlActions>().invoke_activity();
                }
            }
            Self::Settings(window) => {
                if let Some(window) = window.upgrade() {
                    window.set_font_picker_input_shield(false);
                    window.set_font_picker_open(false);
                    window
                        .window()
                        .with_winit_window(|window| window.focus_window());
                }
            }
        }
    }

    fn apply(&self, font: String) -> Result<(), String> {
        match self {
            Self::Player(window, engine) => {
                if window.upgrade().is_some_and(|window| window.get_loaded()) {
                    engine
                        .send(Command::SubtitleFont(font))
                        .map_err(String::from)?;
                }
            }
            Self::Settings(window) => {
                if let Some(window) = window.upgrade() {
                    window
                        .global::<crate::SubtitleSettingsState>()
                        .set_subtitle_font(font.into());
                    window.invoke_settings_changed();
                }
            }
        }
        Ok(())
    }

    fn report(&self, message: String) {
        match self {
            Self::Player(window, _) => {
                if let Some(window) = window.upgrade() {
                    crate::diagnostics::report(&window, message);
                }
            }
            Self::Settings(window) => {
                if let Some(window) = window.upgrade() {
                    window.set_profile_status(message.into());
                }
            }
        }
    }
}

fn hide(picker: &FontPicker) {
    picker.set_picker_active(false);
    let _ = picker.hide();
}

fn set_theme(picker: &FontPicker, dark: bool) {
    picker.set_dark_theme(dark);
    let theme = if dark { Theme::Dark } else { Theme::Light };
    picker
        .window()
        .with_winit_window(|window| window.set_theme(Some(theme)));
}

pub(crate) fn font_name(value: SharedString) -> Result<String, String> {
    match font_from_text(value.as_str()).map_err(|error| error.to_string())? {
        Command::SubtitleFont(font) => Ok(font),
        _ => Err("Invalid subtitle font".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, sync::mpsc::TryRecvError};

    #[test]
    fn font_picker_routes_one_retained_dialog_to_its_current_owner() {
        i_slint_backend_testing::init_no_event_loop();
        let service = Rc::new(ApplicationFontPicker::default());
        let first = PlayerWindow::new().unwrap();
        let second = PlayerWindow::new().unwrap();
        let (first_sender, first_engine) = CommandSender::recording(8);
        let (second_sender, second_engine) = CommandSender::recording(8);
        service.connect_player(&first, &first_sender);
        service.connect_player(&second, &second_sender);
        let choose = |player: &PlayerWindow| {
            player
                .global::<PlayerMenuActions>()
                .invoke_choose_subtitle_font();
        };

        choose(&first);
        assert!(service.visible_picker().is_none());
        first.set_loaded(true);
        choose(&first);
        let picker = service.visible_picker().unwrap();
        picker.set_preview_text("Personal sample".into());
        picker.set_filter_text("sans".into());
        picker.invoke_accept_font("Verdana".into());
        assert_eq!(
            first_engine.commands.try_recv(),
            Ok(Command::SubtitleFont("Verdana".into()))
        );

        second.set_loaded(true);
        choose(&second);
        first.invoke_dismiss_font_picker();
        let picker = service.visible_picker().unwrap();
        assert_eq!(picker.get_preview_text(), "Personal sample");
        assert_eq!(picker.get_filter_text(), "sans");
        picker.invoke_cancel();
        assert_eq!(second_engine.commands.try_recv(), Err(TryRecvError::Empty));
        assert!(!picker.get_picker_active());

        let settings = PreferencesWindow::new().unwrap();
        let selected = settings.global::<crate::SubtitleSettingsState>();
        selected.set_subtitle_font("Saved family".into());
        let saved = Rc::new(Cell::new(0));
        let edits = saved.clone();
        settings.on_settings_changed(move || edits.set(edits.get() + 1));
        service.connect_settings(&settings);
        settings.invoke_choose_subtitle_font();
        assert!(settings.get_font_picker_open());
        assert!(settings.get_font_picker_input_shield());
        assert_eq!(picker.get_custom_name(), "Saved family");
        choose(&first);
        picker.invoke_accept_font("Settings family".into());
        assert_eq!(selected.get_subtitle_font(), "Settings family");
        assert!(!settings.get_font_picker_open());
        assert!(!settings.get_font_picker_input_shield());
        assert_eq!(saved.get(), 1);
        assert_eq!(first_engine.commands.try_recv(), Err(TryRecvError::Empty));
        settings.invoke_choose_subtitle_font();
        settings.invoke_close_profile_font_picker();
        assert!(!picker.get_picker_active());
        assert_eq!(saved.get(), 1);
    }
}
