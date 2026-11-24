//! Native extensions to the shared GL surface's renderer lifecycle.
use crate::PlayerWindow;
use riina_mpv::{Player, video::VideoSurface};
use std::{
    cell::RefCell,
    ffi::{CStr, c_void},
    rc::Rc,
};

#[cfg(target_os = "macos")]
mod display_color;

#[derive(Default)]
pub(crate) struct SharedVideoExtras {
    #[cfg(target_os = "macos")]
    color: display_color::DisplayColor,
    #[cfg(target_os = "macos")]
    picture_in_picture: Rc<RefCell<Option<super::MacPictureInPicture>>>,
}

impl SharedVideoExtras {
    #[cfg(target_os = "macos")]
    pub(crate) fn with_picture_in_picture(
        picture_in_picture: Rc<RefCell<Option<super::MacPictureInPicture>>>,
    ) -> Self {
        Self {
            color: display_color::DisplayColor::default(),
            picture_in_picture,
        }
    }

    /// # Safety
    /// The player's rendering setup must keep its own GL context current.
    pub(crate) unsafe fn configure_rendering(
        &self,
        loader: &dyn Fn(&CStr) -> *const c_void,
        surface: &Rc<RefCell<Option<VideoSurface>>>,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        if let Some(manager) = self.picture_in_picture.borrow_mut().as_mut() {
            // SAFETY: the caller holds the current CGL context for this setup.
            return unsafe { manager.configure_rendering(loader, Rc::downgrade(surface)) };
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (loader, surface);
        Ok(())
    }

    pub(crate) fn update_display_color(
        &mut self,
        app: &PlayerWindow,
        video: &mut VideoSurface,
        player: &Player,
    ) {
        #[cfg(target_os = "macos")]
        self.color.update(app, video, player);
        #[cfg(not(target_os = "macos"))]
        let _ = (app, video, player);
    }

    /// # Safety
    /// Call with the GL context current for the supplied video surface.
    pub(crate) unsafe fn render_pip(&self, video: &mut VideoSurface) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        if let Some(manager) = self.picture_in_picture.borrow_mut().as_mut() {
            // SAFETY: the shared surface and PiP renderer use this current CGL context.
            return unsafe { manager.render_frame(video) }.map(|_| ());
        }
        #[cfg(not(target_os = "macos"))]
        let _ = video;
        Ok(())
    }

    /// # Safety
    /// Call before the player's GL context is released.
    pub(crate) unsafe fn teardown_rendering(&self) {
        #[cfg(target_os = "macos")]
        if let Some(manager) = self.picture_in_picture.borrow_mut().as_mut() {
            // SAFETY: the caller keeps the original CGL context current through teardown.
            unsafe { manager.teardown_rendering() };
        }
    }

    pub(crate) fn reset_display_color(&mut self) {
        #[cfg(target_os = "macos")]
        {
            self.color = display_color::DisplayColor::default();
        }
    }
}
