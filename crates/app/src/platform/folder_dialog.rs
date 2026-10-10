//! Dialogs retain their real native owner until selection or cancellation.
use slint::winit_030::winit::window::Window;
use std::sync::Arc;

pub(crate) fn pick(
    owner: Arc<Window>,
    complete: impl FnOnce(Result<Option<String>, String>) + Send + 'static,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        let token = super::file_dialog::DialogOwner::new(owner.clone())?;
        owner.set_enable(false);
        let restore = owner.clone();
        let started = std::thread::Builder::new()
            .name("screenshot-folder-picker".into())
            .spawn(move || {
                let result = super::file_dialog::pick_folder(&token);
                let _ = slint::invoke_from_event_loop(move || {
                    owner.set_enable(true);
                    owner.focus_window();
                    complete(result);
                });
            });
        if let Err(error) = started {
            restore.set_enable(true);
            return Err(error.to_string());
        }
        Ok(())
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        slint::spawn_local(async move {
            let selection = rfd::AsyncFileDialog::new()
                .set_title("Choose screenshot save path")
                .set_parent(owner.as_ref())
                .pick_folder()
                .await;
            let result = selection
                .map(|folder| {
                    folder
                        .path()
                        .to_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "The path contains invalid Unicode".to_owned())
                })
                .transpose();
            complete(result);
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
    }
}
