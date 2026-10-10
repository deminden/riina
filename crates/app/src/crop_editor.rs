//! UI coordinator; queued crop edits retain their original source identity.
use crate::PlayerWindow;
use riina_core::{
    Command, Playback,
    crop::Rectangle,
    crop_geometry::{DisplayRect, Point, Projection},
    crop_session::Session,
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
    rectangle: Option<Rectangle>,
    drag: Option<(i32, Point, DisplayRect)>,
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
        Projection::for_video(
            video,
            DisplayRect {
                x: 0.0,
                y: 0.0,
                width: f64::from(app.get_video_width()),
                height: f64::from(app.get_video_height()),
            },
        )
    }

    fn refresh(&self, app: &PlayerWindow) {
        let (editing, ready, rectangle, screen) = {
            let data = self.0.lock().expect("Crop editor state is available");
            let ready = data
                .session
                .as_ref()
                .zip(data.latest.as_ref())
                .is_some_and(|(session, state)| session.ready(state) && !state.seeking);
            let screen = data
                .rectangle
                .and_then(|rectangle| Self::projection(&data, app).ok()?.project(rectangle).ok());
            (data.session.is_some(), ready, data.rectangle, screen)
        };
        // Property changes may call back into layout. Never hold the coordinator
        // mutex while publishing UI properties or moving keyboard focus.
        let entering = !app.get_crop_editing() && editing;
        let exiting = app.get_crop_editing() && !editing;
        app.set_crop_editing(editing);
        app.set_crop_ready(ready && screen.is_some());
        if exiting {
            app.invoke_focus_player();
        }
        if entering {
            app.set_sidebar(0);
            app.set_crop_preset_index(-1);
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
            data.rectangle = Some(session.whole_source());
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
            let rectangle = if confirm { data.rectangle } else { None };
            session.exit_commands(state, rectangle)?;
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
                    data.rectangle = Some(Rectangle {
                        x: ((f64::from(whole.width) - width) / 2.0) as u32,
                        y: ((f64::from(whole.height) - height) / 2.0) as u32,
                        width: width as u32,
                        height: height as u32,
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
                    && let Ok(screen) = projection.project(rectangle)
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
                        data.drag = Some((kind, point, screen));
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
                    if let Some((kind, start, screen)) = data.drag
                        && let Ok(projection) = Self::projection(&data, &app)
                    {
                        let point = Point {
                            x: f64::from(x),
                            y: f64::from(y),
                        };
                        let left = screen.x;
                        let top = screen.y;
                        let right = left + screen.width;
                        let bottom = top + screen.height;
                        let (start, end) = match kind {
                            1 => (
                                Point {
                                    x: point.x.min(right),
                                    y: top,
                                },
                                Point {
                                    x: right,
                                    y: bottom,
                                },
                            ),
                            2 => (
                                Point { x: left, y: top },
                                Point {
                                    x: point.x.max(left),
                                    y: bottom,
                                },
                            ),
                            3 => (
                                Point {
                                    x: left,
                                    y: point.y.min(bottom),
                                },
                                Point {
                                    x: right,
                                    y: bottom,
                                },
                            ),
                            4 => (
                                Point { x: left, y: top },
                                Point {
                                    x: right,
                                    y: point.y.max(top),
                                },
                            ),
                            _ => (start, point),
                        };
                        if let Ok(rectangle) = projection.select(start, end) {
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
