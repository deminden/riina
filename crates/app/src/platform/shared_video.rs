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
    #[cfg(target_os = "linux")]
    picture_in_picture: Option<Rc<super::unix::pip::LinuxPictureInPicture>>,
}

impl SharedVideoExtras {
    #[cfg(target_os = "linux")]
    pub(crate) fn with_linux_picture_in_picture(
        picture_in_picture: Rc<super::unix::pip::LinuxPictureInPicture>,
    ) -> Self {
        Self {
            picture_in_picture: Some(picture_in_picture),
        }
    }

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
        #[cfg(target_os = "linux")]
        if let Some(picture_in_picture) = &self.picture_in_picture {
            // SAFETY: the Linux relay is allocated on the player's creating context.
            return unsafe {
                picture_in_picture.configure_rendering(loader, Rc::downgrade(surface))
            };
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
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
        #[cfg(target_os = "linux")]
        if let Some(picture_in_picture) = &self.picture_in_picture {
            // SAFETY: the relay uses the creating context held by the draw hook.
            return unsafe { picture_in_picture.render_frame(video) };
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let _ = video;
        Ok(())
    }

    /// # Safety
    /// Call before the player's GL context is released.
    pub(crate) unsafe fn teardown_rendering(&self) {
        #[cfg(target_os = "linux")]
        if let Some(picture_in_picture) = &self.picture_in_picture {
            // SAFETY: teardown still holds the context that allocated the transfer buffers.
            unsafe { picture_in_picture.teardown_rendering() };
        }
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
