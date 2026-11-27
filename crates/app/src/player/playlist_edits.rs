//! Publish playlist additions in submission order, with one queue per player.
use super::{media_open, player_ui::ObserverId};
use crate::{PlayerWindow, background, diagnostics, file_selection};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy)]
pub(super) enum Insertion {
    Append,
    Before(Option<i64>),
}

type Prepared = Result<Command, String>;

struct Slot {
    id: u64,
    command: Option<Prepared>,
}

#[derive(Default)]
pub(in crate::player) struct Queue {
    slots: VecDeque<Slot>,
    next: u64,
    retired: Arc<AtomicBool>,
    replacing_media: bool,
}

impl Queue {
    pub(super) fn has_pending_media(&self) -> bool {
        self.replacing_media || !self.slots.is_empty()
    }

    fn reserve(&mut self, command: Option<Prepared>) -> Result<u64, String> {
        if self.slots.len() >= 32 {
            return Err("Too many playlist additions are pending".into());
        }
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        self.slots.push_back(Slot { id, command });
        Ok(id)
    }

    fn complete(&mut self, id: u64, command: Prepared) -> Vec<Prepared> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) else {
            return Vec::new();
        };
        slot.command = Some(command);
        self.take_ready()
    }

    fn take_ready(&mut self) -> Vec<Prepared> {
        let mut ready = Vec::new();
        if !self.replacing_media {
            while let Some(command) = self.slots.front_mut().and_then(|slot| slot.command.take()) {
                self.slots.pop_front();
                ready.push(command);
            }
        }
        ready
    }

    fn retire(&mut self) {
        self.retired.store(true, Ordering::Relaxed);
        self.retired = Arc::default();
        self.slots.clear();
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        self.retired.store(true, Ordering::Relaxed);
    }
}

fn queue(app: &PlayerWindow) -> Option<Rc<RefCell<Queue>>> {
    ObserverId::for_window(app)?.playlist_edits()
}

pub(super) fn retire_media(app: &PlayerWindow) {
    if let Some(queue) = queue(app) {
        let mut queue = queue.borrow_mut();
        queue.retire();
        queue.replacing_media = false;
    }
}

pub(super) fn hold_for_open(app: &PlayerWindow) {
    if let Some(queue) = queue(app) {
        queue.borrow_mut().replacing_media = true;
    }
}

pub(super) fn release_after_open(app: &PlayerWindow, sender: &CommandSender) {
    if let Some(queue) = queue(app) {
        let ready = {
            let mut queue = queue.borrow_mut();
            queue.replacing_media = false;
            queue.take_ready()
        };
        publish(app, sender, ready);
    }
}

pub(super) fn clear(app: &PlayerWindow, sender: &CommandSender) -> Result<(), String> {
    sender
        .send_playlist_edit(Command::ClearPlaylist)
        .map_err(String::from)?;
    if let Some(queue) = queue(app) {
        queue.borrow_mut().retire();
    }
    Ok(())
}

pub(super) fn submit(
    app: &PlayerWindow,
    sender: &CommandSender,
    command: Command,
) -> Result<(), String> {
    let observer = ObserverId::for_window(app);
    if let Some(queue) = observer.and_then(ObserverId::playlist_edits) {
        let mut queue = queue.borrow_mut();
        if queue.replacing_media || !queue.slots.is_empty() {
            queue.reserve(Some(Ok(command)))?;
            if let Some(observer) = observer {
                observer.supersede_media();
            }
            return Ok(());
        }
    }
    sender.send_playlist_edit(command).map_err(String::from)?;
    if let Some(observer) = observer {
        observer.supersede_media();
    }
    Ok(())
}

pub(crate) fn add_files(
    app: &PlayerWindow,
    sender: &CommandSender,
    paths: Vec<String>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    prepare(app, sender, Insertion::Append, move || Ok(paths))
}

pub(super) fn prepare(
    app: &PlayerWindow,
    sender: &CommandSender,
    insertion: Insertion,
    references: impl FnOnce() -> Result<Vec<String>, String> + Send + 'static,
) -> Result<(), String> {
    let observer = ObserverId::for_window(app).ok_or("The player is closed")?;
    let queue = observer.playlist_edits().ok_or("The player is closed")?;
    let (id, retired) = {
        let mut queue = queue.borrow_mut();
        (queue.reserve(None)?, queue.retired.clone())
    };
    observer.supersede_media();
    let window = app.as_weak();
    let (ready, result) = tokio::sync::oneshot::channel();
    let commands = sender.clone();
    let weak_queue = Rc::downgrade(&queue);
    if let Err(error) = slint::spawn_local(async move {
        let result = result
            .await
            .unwrap_or_else(|_| Err("Could not prepare playlist files".into()));
        if let (Some(app), Some(queue)) = (window.upgrade(), weak_queue.upgrade()) {
            let ready = queue.borrow_mut().complete(id, result);
            publish(&app, &commands, ready);
        }
    }) {
        queue.borrow_mut().slots.pop_back();
        return Err(error.to_string());
    }
    if let Err(error) = background::spawn_named("riina-playlist-insertion", move || {
        let cancelled = || retired.load(Ordering::Relaxed);
        if !cancelled() {
            let value = references().and_then(|paths| prepare_command(paths, insertion, cancelled));
            let _ = ready.send(value);
        }
    }) {
        let ready = queue.borrow_mut().complete(id, Err(error.to_string()));
        publish(app, sender, ready);
    }
    Ok(())
}

fn prepare_command(
    paths: Vec<String>,
    insertion: Insertion,
    cancelled: impl Fn() -> bool,
) -> Prepared {
    let paths = media_open::resolve_url_shortcuts(paths, &cancelled)?;
    let paths = file_selection::prepare_cancellable(paths, false, cancelled)?;
    if paths.is_empty() {
        return Err("No playable files were found for the playlist".into());
    }
    Ok(match insertion {
        Insertion::Append => Command::AppendFiles(paths),
        Insertion::Before(before_id) => Command::InsertPlaylistFilesBefore { paths, before_id },
    })
}

fn publish(app: &PlayerWindow, sender: &CommandSender, ready: Vec<Prepared>) {
    for result in ready {
        if let Err(error) =
            result.and_then(|command| sender.send_playlist_edit(command).map_err(String::from))
        {
            diagnostics::report(app, error);
        }
    }
}

#[cfg(test)]
pub(in crate::player) mod tests {
    use super::*;
    use crate::{player_ui::glue_tests::Harness, ui_dialogs};
    use riina_test_support::TempDir;
    use riina_ui::{PlayerDialogState, PlayerMenuActions};

    #[test]
    fn playlist_sources_keep_folder_order_and_share_insertion_rules() {
        let directory = TempDir::new("playlist-folder").unwrap();
        for name in ["clip10.avi", "clip2.avi", "clip2.srt", "all.m3u8"] {
            std::fs::write(directory.path().join(name), []).unwrap();
        }
        let paths = vec![directory.path().to_str().unwrap().to_owned()];
        let expected = ["clip2.avi", "clip10.avi"]
            .map(|name| directory.path().join(name).to_str().unwrap().to_owned())
            .to_vec();
        assert_eq!(
            prepare_command(paths.clone(), Insertion::Append, || false),
            Ok(Command::AppendFiles(expected.clone()))
        );
        assert_eq!(
            prepare_command(paths.clone(), Insertion::Before(Some(42)), || false),
            Ok(Command::InsertPlaylistFilesBefore {
                paths: expected,
                before_id: Some(42),
            })
        );
        assert!(prepare_command(paths, Insertion::Append, || true).is_err());
    }

    pub(in crate::player) fn exercise_playlist_edit_order(
        profile: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let player = Harness::start(profile.join("playlist-edit-order/preferences.json"), false)?;
        let app = &player.app;
        let commands = &player.sender;
        let edits = queue(app).unwrap();
        let dialog = app.global::<PlayerDialogState>();
        let url = "https://example.test/clip.mkv";

        // An earlier folder is still preparing when a URL is submitted and
        // the drop below it has already been resolved.
        hold_for_open(app);
        let first = edits.borrow_mut().reserve(None)?;
        assert!(dialog.invoke_append_playlist_url(url.into()));
        let third = edits.borrow_mut().reserve(None)?;
        let inserted = Command::InsertPlaylistFilesBefore {
            paths: vec!["dropped.avi".into()],
            before_id: Some(42),
        };
        assert!(
            edits
                .borrow_mut()
                .complete(third, Ok(inserted.clone()))
                .is_empty()
        );
        let folder = Command::AppendFiles(vec!["folder/clip.mkv".into()]);
        assert!(
            edits
                .borrow_mut()
                .complete(first, Ok(folder.clone()))
                .is_empty()
        );
        assert!(player.engine.commands.try_recv().is_err());

        let opened = Command::OpenFiles(vec!["opening.mkv".into()]);
        commands.send(opened.clone())?;
        release_after_open(app, commands);
        assert_eq!(
            player.engine.commands.try_iter().collect::<Vec<_>>(),
            [
                opened,
                folder,
                Command::AppendFiles(vec![url.into()]),
                inserted
            ]
        );

        let pending = edits.borrow_mut().reserve(None)?;
        let retired = edits.borrow().retired.clone();
        app.global::<PlayerMenuActions>()
            .invoke_playlist_action(4, "".into());
        assert_eq!(player.engine.commands.try_recv()?, Command::ClearPlaylist);
        assert!(retired.load(Ordering::Relaxed));
        assert!(
            edits
                .borrow_mut()
                .complete(pending, Err("late scan".into()))
                .is_empty()
        );

        // Both native playlist picker responses go through the real dispatcher.
        let media_generation = app.get_media_request_generation();
        for purpose in [
            ui_dialogs::Purpose::Playlist,
            ui_dialogs::Purpose::PlaylistFolders,
        ] {
            ui_dialogs::dispatch_selection(app, purpose, vec![url.into()], commands.clone())?;
        }
        assert_eq!(app.get_media_request_generation(), media_generation);
        let retired = edits.borrow().retired.clone();
        media_open::invalidate(app);
        assert!(retired.load(Ordering::Relaxed));
        assert!(edits.borrow().slots.is_empty());
        assert!(dialog.invoke_append_playlist_url(url.into()));
        assert_eq!(
            player.engine.commands.try_recv()?,
            Command::AppendFiles(vec![url.into()])
        );
        assert!(player.engine.commands.try_recv().is_err());
        Ok(())
    }
}
