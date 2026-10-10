//! UI coordinator; queued crop edits retain their original source identity.
use crate::PlayerWindow;
use riina_core::{
    Command, Playback,
    crop_drag::{Drag, Edge},
    crop_geometry::{Point, Projection, SourceRect},
    crop_session::Session,
    video_viewport::Viewport,
};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct Data {
    latest: Option<Playback>,
    session: Option<Session>,
    rectangle: Option<SourceRect>,
    drag: Option<Drag>,
    sender: Option<CommandSender>,
    viewport: Option<Option<Viewport>>,
}

#[derive(Clone, Default)]
pub struct Editor(Arc<Mutex<Data>>);

impl Editor {
    pub fn observe(&self, app: &PlayerWindow, state: &Playback) {
        {
            let mut data = self.0.lock().expect("Crop editor state is available");
            if data
                .session
                .as_ref()
                .is_some_and(|session| session.validate(state).is_err())
            {
                data.session = None;
                data.rectangle = None;
                data.drag = None;
            }
            data.latest = Some(state.clone());
        }
        self.refresh(app);
    }

    fn projection(data: &Data, app: &PlayerWindow) -> Result<Projection, String> {
        let video = data
            .latest
            .as_ref()
            .and_then(|state| state.video.as_ref())
            .ok_or("Source video is unavailable")?;
        Projection::interactive(
            video,
            (
                f64::from(app.get_video_width()),
                f64::from(app.get_video_height()),
            ),
        )
    }

    fn refresh(&self, app: &PlayerWindow) {
        let (editing, ready, rectangle, screen, content) = {
            let data = self.0.lock().expect("Crop editor state is available");
            let ready = data
                .session
                .as_ref()
                .zip(data.latest.as_ref())
                .is_some_and(|(session, state)| session.ready(state) && !state.seeking);
            let screen = data.rectangle.and_then(|rectangle| {
                Self::projection(&data, app)
                    .ok()?
                    .project_continuous(rectangle)
                    .ok()
            });
            let rectangle = data.rectangle.and_then(|rectangle| {
                let whole = data.session.as_ref()?.whole_source();
                rectangle.to_native((whole.width, whole.height)).ok()
            });
            let content = Self::projection(&data, app)
                .ok()
                .map(|projection| projection.content());
            (data.session.is_some(), ready, rectangle, screen, content)
        };
        // Property changes may call back into layout. Never hold the coordinator
        // mutex while publishing UI properties or moving keyboard focus.
        let entering = !app.get_crop_editing() && editing;
        let exiting = app.get_crop_editing() && !editing;
        if let Some(content) = content {
            app.set_crop_picture_x(content.x as f32);
            app.set_crop_picture_y(content.y as f32);
            app.set_crop_picture_width(content.width as f32);
            app.set_crop_picture_height(content.height as f32);
        }
        app.set_crop_editing(editing);
        app.set_crop_ready(ready && screen.is_some());
        if app.get_native_video() {
            let viewport = if editing {
                content.and_then(|content| {
                    Viewport::new(
                        (
                            f64::from(app.get_video_width()),
                            f64::from(app.get_video_height()),
                        ),
                        content,
                    )
                    .ok()
                })
            } else {
                None
            };
            let mut data = self.0.lock().expect("Crop editor state is available");
            if data.viewport != Some(viewport)
                && let Some(sender) = data.sender.as_ref()
            {
                match sender.send(Command::VideoViewport(viewport)) {
                    Ok(()) => data.viewport = Some(viewport),
                    Err(error) => {
                        drop(data);
                        app.set_status(error.into());
                    }
                }
            }
        }
        if exiting {
            app.invoke_focus_player();
        }
        if entering {
            app.set_sidebar(0);
            app.set_leading_sidebar(0);
            app.set_seek_osd_visible(false);
            app.set_video_osd_visible(false);
            let weak = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(32), move || {
                if let Some(app) = weak.upgrade()
                    && app.get_crop_editing()
                {
                    app.invoke_focus_crop_editor();
                }
            });
        }
        if let Some(rectangle) = rectangle {
            app.set_crop_description(
                format!(
                    "({}, {}) ({}×{})",
                    rectangle.x, rectangle.y, rectangle.width, rectangle.height
                )
                .into(),
            );
        }
        if let Some(screen) = screen {
            app.set_crop_box_x(screen.x as f32);
            app.set_crop_box_y(screen.y as f32);
            app.set_crop_box_width(screen.width as f32);
            app.set_crop_box_height(screen.height as f32);
        }
    }

    fn begin(&self, app: &PlayerWindow, sender: &CommandSender) -> Result<(), String> {
        {
            let mut data = self.0.lock().expect("Crop editor state is available");
            if data.session.is_some() {
                return Ok(());
            }
            let state = data.latest.as_ref().ok_or("Source video is unavailable")?;
            Self::projection(&data, app)?;
            let session = Session::begin(state)?;
            sender.send(Command::EnterCrop(session.clone()))?;
            data.rectangle = Some(session.whole_source().into());
            data.session = Some(session);
            data.drag = None;
        }
        self.refresh(app);
        Ok(())
    }

    fn finish(
        &self,
        app: &PlayerWindow,
        sender: &CommandSender,
        confirm: bool,
    ) -> Result<(), String> {
        {
            let mut data = self.0.lock().expect("Crop editor state is available");
            let Some(session) = data.session.as_ref() else {
                return Ok(());
            };
            let state = data.latest.as_ref().ok_or("Source video is unavailable")?;
            if confirm && !session.ready(state) {
                return Err("The video is not ready for crop confirmation".into());
            }
            let rectangle = if confirm {
                let whole = session.whole_source();
                Some(
                    data.rectangle
                        .ok_or("The crop selection is unavailable")?
                        .to_native((whole.width, whole.height))?,
                )
            } else {
                None
            };
            session.exit_commands(state, rectangle)?;
            if app.get_native_video() {
                // Exit can resume a playing source. Restore its full picture
                // area first so that resumed frames do not use editor margins.
                sender.send(Command::VideoViewport(None))?;
            }
            sender.send(Command::ExitCrop {
                session: session.clone(),
                rectangle,
            })?;
            data.session = None;
            data.rectangle = None;
            data.drag = None;
        }
        self.refresh(app);
        Ok(())
    }

    pub fn install(&self, app: &PlayerWindow, sender: CommandSender) {
        self.0
            .lock()
            .expect("Crop editor state is available")
            .sender = Some(sender.clone());
        let editor = self.clone();
        let weak = app.as_weak();
        let begin_sender = sender.clone();
        app.on_begin_custom_crop(move || {
            if let Some(app) = weak.upgrade()
                && let Err(error) = editor.begin(&app, &begin_sender)
            {
                app.set_status(error.into());
            }
        });
        let editor = self.clone();
        let weak = app.as_weak();
        app.on_finish_custom_crop(move |confirm| {
            if let Some(app) = weak.upgrade()
                && let Err(error) = editor.finish(&app, &sender, confirm)
            {
                app.set_status(error.into());
            }
        });
        let editor = self.clone();
        let weak = app.as_weak();
        app.on_crop_viewport_changed(move || {
            if let Some(app) = weak.upgrade() {
                editor.refresh(&app);
            }
        });
        let editor = self.clone();
        let weak = app.as_weak();
        app.on_crop_preset(move |index| {
            if let Some(app) = weak.upgrade() {
                {
                    let mut data = editor.0.lock().expect("Crop editor state is available");
                    let Some(session) = data.session.as_ref() else {
                        return;
                    };
                    let whole = session.whole_source();
                    let Some((w, h)) = usize::try_from(index)
                        .ok()
                        .and_then(|index| {
                            [
                                (4.0, 3.0),
                                (16.0, 9.0),
                                (16.0, 10.0),
                                (21.0, 9.0),
                                (3.0, 2.0),
                                (5.0, 4.0),
                            ]
                            .get(index)
                        })
                        .copied()
                    else {
                        return;
                    };
                    let ratio = w / h;
                    let width = f64::from(whole.width).min(f64::from(whole.height) * ratio);
                    let height = f64::from(whole.height).min(f64::from(whole.width) / ratio);
                    data.rectangle = Some(SourceRect {
                        x: (f64::from(whole.width) - width) / 2.0,
                        y: (f64::from(whole.height) - height) / 2.0,
                        width,
                        height,
                    });
                }
                editor.refresh(&app);
            }
        });
        let editor = self.clone();
        let weak = app.as_weak();
        app.on_begin_crop_drag(move |kind, x, y| {
            if let Some(app) = weak.upgrade()
                && app.get_crop_ready()
            {
                let mut data = editor.0.lock().expect("Crop editor state is available");
                if let Ok(projection) = Self::projection(&data, &app)
                    && let Some(rectangle) = data.rectangle
                    && let Ok(screen) = projection.project_continuous(rectangle)
                {
                    let point = Point {
                        x: f64::from(x),
                        y: f64::from(y),
                    };
                    let content = projection.content();
                    if (1..=4).contains(&kind)
                        || (point.x >= content.x
                            && point.x <= content.x + content.width
                            && point.y >= content.y
                            && point.y <= content.y + content.height)
                    {
                        let edge = match kind {
                            0 => None,
                            1 => Some(Edge::Left),
                            2 => Some(Edge::Right),
                            3 => Some(Edge::Top),
                            4 => Some(Edge::Bottom),
                            _ => return,
                        };
                        data.drag = Drag::begin(edge, point, screen).ok();
                    }
                }
            }
        });
        let editor = self.clone();
        let weak = app.as_weak();
        app.on_move_crop_drag(move |x, y| {
            if let Some(app) = weak.upgrade() {
                {
                    let mut data = editor.0.lock().expect("Crop editor state is available");
                    if let Some(drag) = data.drag
                        && let Ok(projection) = Self::projection(&data, &app)
                    {
                        let point = Point {
                            x: f64::from(x),
                            y: f64::from(y),
                        };
                        if let Ok((start, end)) = drag.points(point)
                            && let Ok(rectangle) = projection.select_continuous(start, end)
                        {
                            data.rectangle = Some(rectangle);
                        }
                    }
                }
                editor.refresh(&app);
            }
        });
        let editor = self.clone();
        app.on_end_crop_drag(move || {
            editor
                .0
                .lock()
                .expect("Crop editor state is available")
                .drag = None;
        });
    }
}
