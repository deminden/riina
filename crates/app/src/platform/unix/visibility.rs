//! Linux window managers minimize individual toplevels, so Hide Riina visits our own windows.

use slint::{ComponentHandle, Window};
use std::{cell::RefCell, collections::BTreeMap, ptr};

type WindowVisit = Box<dyn Fn(&mut dyn FnMut(&Window)) -> bool>;

thread_local! {
    static WINDOWS: RefCell<BTreeMap<*const Window, WindowVisit>> = const {
        RefCell::new(BTreeMap::new())
    };
}

pub(crate) fn track(window: &(impl ComponentHandle + 'static)) {
    let identity = ptr::from_ref(window.window());
    let weak = window.as_weak();
    WINDOWS.with_borrow_mut(|windows| {
        // Reopening the same window replaces its registration. Released
        // components cannot keep resources alive through this registry.
        windows.retain(|_, visit| visit(&mut |_| {}));
        windows.insert(
            identity,
            Box::new(move |visit| {
                let Some(component) = weak.upgrade() else {
                    return false;
                };
                let window = component.window();
                if window.is_visible() {
                    visit(window);
                }
                true
            }),
        );
    });
}

pub(crate) fn minimize_all() {
    visit_visible(super::super::minimize_window);
}

fn visit_visible(mut visit: impl FnMut(&Window)) {
    WINDOWS.with_borrow_mut(|windows| windows.retain(|_, window| window(&mut visit)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerWindow, PreferencesWindow, WelcomeWindow, platform::show_window};

    #[test]
    fn hide_targets_distinct_visible_player_and_auxiliary_windows()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let settings = PreferencesWindow::new()?;
        let welcome = WelcomeWindow::new()?;
        show_window(&first)?;
        show_window(&second)?;
        show_window(&settings)?;
        show_window(&settings)?;
        show_window(&welcome)?;
        welcome.hide()?;

        let mut targets = Vec::new();
        visit_visible(|window| targets.push(ptr::from_ref(window)));
        assert_eq!(targets.len(), 3);
        assert!(targets.contains(&ptr::from_ref(first.window())));
        assert!(targets.contains(&ptr::from_ref(second.window())));
        assert!(targets.contains(&ptr::from_ref(settings.window())));

        settings.hide()?;
        drop(settings);
        visit_visible(|_| {});
        assert_eq!(WINDOWS.with_borrow(|windows| windows.len()), 3);
        Ok(())
    }
}
