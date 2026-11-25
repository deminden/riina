//! Pending open work acts as a barrier to closing an empty application.

#[derive(Default)]
pub(super) struct WindowLifecycle {
    documents: usize,
    picker: bool,
    deferred_close: bool,
}

pub(super) enum EmptyAction {
    Wait,
    IdleMenu,
    FinishClose,
}

impl WindowLifecycle {
    pub(super) fn player_attached(&mut self) {
        self.deferred_close = false;
    }

    pub(super) fn after_last_player_close(
        &mut self,
        tools_visible: bool,
        player_was_visible: bool,
        has_idle_menu: bool,
    ) -> EmptyAction {
        self.deferred_close |= player_was_visible || !has_idle_menu;
        self.after_barrier(tools_visible, has_idle_menu)
    }

    pub(super) fn after_barrier(
        &mut self,
        tools_visible: bool,
        has_idle_menu: bool,
    ) -> EmptyAction {
        if tools_visible || self.opening() {
            EmptyAction::Wait
        } else if std::mem::take(&mut self.deferred_close) || !has_idle_menu {
            EmptyAction::FinishClose
        } else {
            EmptyAction::IdleMenu
        }
    }

    pub(super) fn opening(&self) -> bool {
        self.picker || self.documents != 0
    }
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy)]
pub(super) enum OpenWork {
    Documents,
    FilePicker,
}

#[cfg(any(target_os = "macos", test))]
impl WindowLifecycle {
    pub(super) fn begin_open(&mut self, work: OpenWork) -> bool {
        match work {
            OpenWork::Documents => self.documents += 1,
            OpenWork::FilePicker if self.picker => return false,
            OpenWork::FilePicker => self.picker = true,
        }
        true
    }

    pub(super) fn finish_open(&mut self, work: OpenWork) -> bool {
        match work {
            OpenWork::Documents => self.documents -= 1,
            OpenWork::FilePicker => self.picker = false,
        }
        self.deferred_close && !self.opening()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_during_finder_delivery_or_picker_waits_for_all_open_work() {
        let mut lifecycle = WindowLifecycle::default();
        assert!(lifecycle.begin_open(OpenWork::FilePicker));
        assert!(!lifecycle.begin_open(OpenWork::FilePicker));
        assert!(lifecycle.begin_open(OpenWork::Documents));
        assert!(matches!(
            lifecycle.after_last_player_close(true, true, true),
            EmptyAction::Wait
        ));
        assert!(matches!(
            lifecycle.after_barrier(false, true),
            EmptyAction::Wait
        ));
        assert!(!lifecycle.finish_open(OpenWork::FilePicker));
        assert!(lifecycle.begin_open(OpenWork::Documents));
        assert!(!lifecycle.finish_open(OpenWork::Documents));
        assert!(lifecycle.finish_open(OpenWork::Documents));
        assert!(matches!(
            lifecycle.after_barrier(false, true),
            EmptyAction::FinishClose
        ));
        assert!(matches!(
            lifecycle.after_barrier(false, true),
            EmptyAction::IdleMenu
        ));
    }

    #[test]
    fn a_replacement_player_retires_the_preceding_deferred_close() {
        let mut lifecycle = WindowLifecycle::default();
        assert!(lifecycle.begin_open(OpenWork::Documents));
        assert!(matches!(
            lifecycle.after_last_player_close(false, true, true),
            EmptyAction::Wait
        ));
        lifecycle.player_attached();
        assert!(!lifecycle.finish_open(OpenWork::Documents));
        assert!(matches!(
            lifecycle.after_barrier(false, true),
            EmptyAction::IdleMenu
        ));
    }

    #[test]
    fn closing_an_application_tool_without_a_player_returns_to_the_idle_menu() {
        let mut lifecycle = WindowLifecycle::default();
        assert!(matches!(
            lifecycle.after_barrier(false, true),
            EmptyAction::IdleMenu
        ));
        assert!(matches!(
            lifecycle.after_barrier(false, false),
            EmptyAction::FinishClose
        ));
        assert!(matches!(
            lifecycle.after_last_player_close(true, true, true),
            EmptyAction::Wait
        ));
        assert!(matches!(
            lifecycle.after_barrier(true, true),
            EmptyAction::Wait
        ));
        assert!(matches!(
            lifecycle.after_barrier(false, true),
            EmptyAction::FinishClose
        ));
    }

    #[test]
    fn a_hidden_player_leaves_the_native_application_open() {
        let mut lifecycle = WindowLifecycle::default();
        assert!(matches!(
            lifecycle.after_last_player_close(false, false, true),
            EmptyAction::IdleMenu
        ));
        assert!(matches!(
            lifecycle.after_last_player_close(false, false, false),
            EmptyAction::FinishClose
        ));
    }
}
