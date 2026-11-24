//! Browser activations have a separate parser and never enter the raw CLI option path.
mod inbox;
pub(crate) use inbox::{Inbox, Registration};

use crate::startup_arguments::{LaunchOptions, Presentation};
use std::{collections::BTreeSet, path::Path};
use url::Url;

pub(crate) const FLAG: &str = "--riina-browser-link";
pub(crate) const MAX_LINK_BYTES: usize = 16 * 1024;

#[derive(Clone)]
pub(crate) struct Link {
    pub(crate) media: String,
    pub(crate) new_window: bool,
    pub(crate) enqueue: bool,
    pub(crate) launch: LaunchOptions,
    pub(crate) speed: Option<f64>,
    pub(crate) muted: Option<bool>,
    pub(crate) position: Option<f64>,
}

impl Link {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        if value.len() > MAX_LINK_BYTES || value.chars().any(char::is_control) {
            return Err("The Riina link is too large or contains invalid characters");
        }
        let link = Url::parse(value).map_err(|_| "The Riina link is malformed")?;
        if link.scheme() != "riina"
            || !matches!(link.host_str(), Some("open" | "weblink"))
            || !matches!(link.path(), "" | "/")
            || link.fragment().is_some()
            || link.port().is_some()
            || link.username() != ""
            || link.password().is_some()
        {
            return Err("The Riina link has an unsupported action");
        }
        let mut fields = BTreeSet::new();
        let mut media = None;
        let mut link_request = Self {
            media: String::new(),
            new_window: false,
            enqueue: false,
            launch: LaunchOptions::default(),
            speed: None,
            muted: None,
            position: None,
        };
        let mut fullscreen = false;
        let mut pip = false;
        for (field, value) in link.query_pairs() {
            if !fields.insert(field.clone()) || value.chars().any(char::is_control) {
                return Err("The Riina link contains an invalid or repeated parameter");
            }
            match field.as_ref() {
                "url" => media = Some(parse_media(&value)?),
                "new_window" => link_request.new_window = switch(&value)?,
                "enqueue" => link_request.enqueue = switch(&value)?,
                "full_screen" => fullscreen = switch(&value)?,
                "pip" => pip = switch(&value)?,
                "mpv_fullscreen" => fullscreen = flag(&value)?,
                "mpv_pause" => link_request.launch.pause = Some(flag(&value)?),
                "mpv_volume" => link_request.launch.volume = Some(number(&value, 0.0, 200.0)?),
                "mpv_speed" => link_request.speed = Some(number(&value, 0.01, 100.0)?),
                "mpv_mute" => link_request.muted = Some(flag(&value)?),
                "mpv_start" => link_request.position = Some(number(&value, 0.0, f64::MAX)?),
                // Browser links are untrusted. Only typed playback options cross this boundary.
                _ => {}
            }
        }
        link_request.media = media.ok_or("The Riina link is missing a media URL")?;
        link_request.launch.presentation = if fullscreen {
            Some(Presentation::Fullscreen)
        } else {
            pip.then_some(Presentation::PictureInPicture)
        };
        Ok(link_request)
    }

    pub(crate) fn initial_media(self) -> crate::player_ui::InitialMedia {
        let Self {
            media,
            mut launch,
            speed,
            muted,
            position,
            ..
        } = self;
        if let Some(speed) = speed {
            launch.engine.push(("speed".into(), speed.to_string()));
        }
        if let Some(muted) = muted {
            launch
                .engine
                .push(("mute".into(), if muted { "yes" } else { "no" }.into()));
        }
        crate::player_ui::InitialMedia {
            files: vec![media.into()],
            resume_position: position,
            launch,
            ..Default::default()
        }
    }

    pub(crate) fn reuse_window(&self, app: &crate::PlayerWindow) -> bool {
        !self.new_window || (self.enqueue && crate::media_open::has_playlist(app))
    }

    pub(crate) fn playback(self, append: bool) -> riina_core::MediaLink {
        riina_core::MediaLink {
            path: self.media,
            append,
            start: self.position,
            pause: self.launch.pause,
            volume: self.launch.volume,
            speed: self.speed,
            mute: self.muted,
        }
    }
}

fn parse_media(media: &str) -> Result<String, &'static str> {
    let parsed = Url::parse(media).ok();
    if let Some(url) = &parsed
        && let Ok(reference) = riina_core::media_reference::validate_network_url(media)
        && url.username().is_empty()
        && url.password().is_none()
        && url.scheme() != "ytdl"
    {
        return Ok(reference.to_owned());
    }
    let local_url = parsed
        .filter(|url| url.scheme() == "file" && url.query().is_none() && url.fragment().is_none())
        .and_then(|url| url.to_file_path().ok());
    let path = local_url.as_deref().unwrap_or_else(|| Path::new(media));
    let path_text = path.to_str().ok_or("The media path is not valid Unicode")?;
    if path.is_absolute()
        && !path_text.starts_with(r"\\")
        && riina_core::file_selection::is_supported_media(path_text)
    {
        return Ok(path_text.to_owned());
    }
    Err("The Riina link must contain a supported network URL or absolute media file")
}

fn switch(value: &str) -> Result<bool, &'static str> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err("The Riina link expects 0 or 1 for window and playlist parameters"),
    }
}

fn flag(value: &str) -> Result<bool, &'static str> {
    match value {
        "no" => Ok(false),
        "yes" => Ok(true),
        _ => Err("A playback option in the Riina link expects yes or no"),
    }
}

fn number(value: &str, minimum: f64, maximum: f64) -> Result<f64, &'static str> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or("A playback option in the Riina link is out of range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_both_actions_and_decodes_nested_url_once() {
        for action in ["open", "weblink"] {
            let request = Link::parse(&format!(
                "riina://{action}?url=https%3A%2F%2Fexample.net%2Fmovie%3Fq%3Da%252Bb%26s%3D1&new_window=1&enqueue=1&pip=1&full_screen=1&mpv_volume=41&mpv_speed=1.25"
            )).unwrap();
            assert_eq!(request.media, "https://example.net/movie?q=a%2Bb&s=1");
            assert!(request.new_window && request.enqueue);
            assert_eq!(request.launch.presentation, Some(Presentation::Fullscreen));
            assert_eq!(request.launch.volume, Some(41.0));
            assert_eq!(request.speed, Some(1.25));
        }
    }

    #[test]
    fn rejects_ambiguous_and_unsupported_media_without_echoing_link_contents() {
        for input in [
            "iina://open?url=https://a.example/first",
            "riina://quit?url=https://a.example/first",
            "riina://open?url=https://a.example/first&url=https://b.example/second",
            "riina://open?url=https://a.example/first%00",
            "riina://open?url=javascript:run",
            "riina://open?url=https://user:secret@a.example/first",
            "riina://open?url=https://a.example/first&mpv_speed=NaN",
        ] {
            assert!(Link::parse(input).is_err());
        }
    }

    #[test]
    fn unsafe_native_options_do_not_reach_the_launch_arguments() {
        let request = Link::parse("riina://open?url=https://a.example/first&mpv_script=run.lua&mpv_ytdl-raw-options=x&mpv_volume=29").unwrap();
        let media = request.initial_media();
        assert_eq!(media.launch.volume, Some(29.0));
        assert!(media.launch.engine.is_empty());
    }
}
