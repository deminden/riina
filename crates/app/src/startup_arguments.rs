//! Turn the process command line into independent player launches.
use crate::player_ui::InitialMedia;
use std::ffi::OsString;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RendererRequest {
    #[default]
    Default,
    SharedGl,
    NativeWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presentation {
    Fullscreen,
    Music,
    PictureInPicture,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LaunchOptions {
    pub(crate) stdin: bool,
    pub(crate) presentation: Option<Presentation>,
    pub(crate) engine: Vec<(String, String)>,
    pub(crate) volume: Option<f64>,
    pub(crate) pause: Option<bool>,
    pub(crate) shuffle: bool,
}

#[derive(Default)]
pub(crate) struct StartupArguments {
    pub(crate) renderer: RendererRequest,
    media: Vec<OsString>,
    resume_position: Option<f64>,
    separate_windows: bool,
    keep_running: bool,
    launch: LaunchOptions,
}

impl StartupArguments {
    pub(crate) fn invocation(
        args: impl Iterator<Item = OsString>,
    ) -> Result<(Self, Option<String>), String> {
        let mut args = args.peekable();
        if args
            .peek()
            .is_some_and(|arg| arg == crate::browser_links::FLAG)
        {
            args.next();
            let raw = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or("Missing Riina browser link")?;
            if args.next().is_some() {
                return Err("A browser activation accepts exactly one Riina link".into());
            }
            let InitialMedia {
                files,
                resume_position,
                launch,
                ..
            } = crate::browser_links::Link::parse(&raw)?.initial_media();
            let arguments = Self {
                media: files,
                resume_position,
                launch,
                ..Default::default()
            };
            return Ok((arguments, Some(raw)));
        }
        Self::parse(args, crate::platform::startup_input::buffered).map(|args| (args, None))
    }

    fn parse(
        args: impl Iterator<Item = OsString>,
        buffered_input: impl FnOnce() -> bool,
    ) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut stdin = None;
        let mut mpv_tail = false;
        let mut music = false;
        let mut pip = false;
        let mut fullscreen = false;
        for argument in args {
            let Some(flag) = argument.to_str() else {
                parsed.media.push(argument);
                continue;
            };
            match flag {
                "--" => mpv_tail = true,
                flag if mpv_tail && flag.starts_with("--") => {
                    fullscreen = parsed.launch.set_mpv(&flag[2..])?.unwrap_or(fullscreen);
                }
                "--shared-gl" if parsed.media.is_empty() => {
                    parsed.renderer = RendererRequest::SharedGl;
                }
                "--native-video" if parsed.media.is_empty() => {
                    parsed.renderer = RendererRequest::NativeWindow;
                }
                "--separate-windows" | "-w" => parsed.separate_windows = true,
                "--stdin" | "-" | "--mpv--" => stdin = Some(true),
                "--no-stdin" => stdin = Some(false),
                "--music-mode" => music = true,
                "--pip" => pip = true,
                "--keep-running" => parsed.keep_running = true,
                flag if flag.starts_with("--mpv-") => {
                    fullscreen = parsed.launch.set_mpv(&flag[6..])?.unwrap_or(fullscreen);
                }
                _ => parsed.media.push(argument),
            }
        }
        parsed.launch.presentation = match (music, pip, fullscreen) {
            (true, true, _) => {
                return Err("Music Mode and Picture in Picture cannot be combined".into());
            }
            (true, false, _) => Some(Presentation::Music),
            (false, true, _) => Some(Presentation::PictureInPicture),
            (false, false, true) => Some(Presentation::Fullscreen),
            (false, false, false) => None,
        };
        parsed.launch.stdin = stdin.unwrap_or_else(buffered_input);
        Ok(parsed)
    }

    pub(crate) fn reads_stdin(&self) -> bool {
        self.launch.stdin
    }

    pub(crate) fn waits_for_player(&self) -> bool {
        self.keep_running || self.reads_stdin()
    }

    pub(crate) fn players(self) -> impl Iterator<Item = InitialMedia> {
        let groups = if self.launch.stdin {
            vec![vec![OsString::from("-")]]
        } else if self.separate_windows && !self.media.is_empty() {
            self.media.into_iter().map(|file| vec![file]).collect()
        } else {
            vec![self.media]
        };
        let final_index = groups.len() - 1;
        groups.into_iter().enumerate().map(move |(index, files)| {
            let mut launch = self.launch.clone();
            if index != final_index && launch.presentation != Some(Presentation::Fullscreen) {
                launch.presentation = None;
            }
            InitialMedia {
                files,
                resume_position: self.resume_position,
                launch,
                ..InitialMedia::default()
            }
        })
    }
}

impl LaunchOptions {
    fn set_mpv(&mut self, flag: &str) -> Result<Option<bool>, String> {
        let (name, value) = flag.split_once('=').unwrap_or_else(|| {
            flag.strip_prefix("no-")
                .map_or((flag, "yes"), |name| (name, "no"))
        });
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("An mpv option name is invalid".into());
        }
        match name {
            "fullscreen" | "fs" => return yes_no(value, "fullscreen").map(Some),
            "pause" => self.pause = Some(yes_no(value, "pause")?),
            "volume" => self.volume = Some(parse_volume(value)?),
            "shuffle" => {
                self.shuffle = yes_no(value, "shuffle")?;
                self.engine.push((name.to_owned(), value.to_owned()));
            }
            "stop-screensaver" | "wid" | "vo" => {
                return Err(format!("Riina manages the mpv {name} option"));
            }
            _ => self.engine.push((name.to_owned(), value.to_owned())),
        }
        Ok(None)
    }
}

fn yes_no(value: &str, name: &str) -> Result<bool, String> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!("The mpv {name} option expects yes or no")),
    }
}

fn parse_volume(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| (0.0..=200.0).contains(value))
        .ok_or_else(|| "The mpv volume option expects a number between 0 and 200".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<StartupArguments, String> {
        StartupArguments::parse(args.iter().map(OsString::from), || false)
    }

    #[test]
    fn buffered_input_is_probed_only_when_input_is_unspecified() {
        let args = StartupArguments::parse(["movie.mkv"].into_iter().map(OsString::from), || true)
            .unwrap();
        assert!(args.waits_for_player());
        assert_eq!(args.players().next().unwrap().files, [OsString::from("-")]);

        for (flag, reads_stdin) in [("--stdin", true), ("--no-stdin", false)] {
            let args = StartupArguments::parse([OsString::from(flag)].into_iter(), || {
                panic!("Unexpected stdin probe for {flag}")
            })
            .unwrap();
            assert_eq!(args.reads_stdin(), reads_stdin);
        }
    }

    #[test]
    fn keep_running_supervises_without_enabling_standard_input() {
        let args = parse(&["--keep-running", "movie.mkv"]).unwrap();
        assert!(args.waits_for_player());
        assert!(!args.reads_stdin());
        assert!(!parse(&["movie.mkv"]).unwrap().waits_for_player());
    }

    #[test]
    fn browser_activation_cannot_supply_raw_command_line_switches() {
        let uri = "riina://open?url=https://example.test/movie.mkv&mpv_volume=24&mpv_start=3.5";
        let browser = [crate::browser_links::FLAG, uri]
            .into_iter()
            .map(OsString::from);
        let (arguments, raw) = StartupArguments::invocation(browser.clone()).unwrap();
        assert_eq!(raw.as_deref(), Some(uri));
        let media = arguments.players().next().unwrap();
        assert_eq!(media.launch.volume, Some(24.0));
        assert_eq!(media.resume_position, Some(3.5));
        assert!(
            StartupArguments::invocation(
                browser.chain([OsString::from("--mpv-script=untrusted.lua")])
            )
            .is_err()
        );
    }

    #[test]
    fn startup_keeps_every_media_argument_after_an_internal_renderer_switch() {
        for (flag, renderer) in [
            ("--shared-gl", RendererRequest::SharedGl),
            ("--native-video", RendererRequest::NativeWindow),
        ] {
            let args = parse(&[flag, "first file.mkv", "second.mkv"]).unwrap();
            assert_eq!(args.renderer, renderer);
            let players = args.players().collect::<Vec<_>>();
            assert_eq!(players.len(), 1);
            assert_eq!(
                players[0].files,
                ["first file.mkv", "second.mkv"].map(OsString::from)
            );
        }
    }

    #[test]
    fn startup_treats_all_arguments_as_media_without_a_renderer_switch() {
        let args = parse(&["a file.mkv", "https://example.net/b.mkv"]).unwrap();
        assert_eq!(args.renderer, RendererRequest::Default);
        let players = args.players().collect::<Vec<_>>();
        assert_eq!(players.len(), 1);
        assert_eq!(
            players[0].files,
            ["a file.mkv", "https://example.net/b.mkv"].map(OsString::from)
        );
    }

    #[test]
    fn separate_windows_share_mpv_options_and_only_the_final_player_enters_pip() {
        let players = parse(&[
            "first.mkv",
            "-w",
            "second.mkv",
            "--pip",
            "--mpv-volume=32",
            "--",
            "--no-resume-playback",
            "--title=title=value",
        ])
        .unwrap()
        .players()
        .collect::<Vec<_>>();
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].files, ["first.mkv"].map(OsString::from));
        assert_eq!(players[1].files, ["second.mkv"].map(OsString::from));
        for player in &players {
            assert_eq!(player.launch.volume, Some(32.0));
            assert_eq!(
                player.launch.engine,
                [
                    ("resume-playback".into(), "no".into()),
                    ("title".into(), "title=value".into())
                ]
            );
        }
        assert_eq!(players[0].launch.presentation, None);
        assert_eq!(
            players[1].launch.presentation,
            Some(Presentation::PictureInPicture)
        );
    }

    #[test]
    fn explicitly_piped_input_takes_precedence_and_can_be_cancelled() {
        let input = parse(&["--stdin", "-w", "ignored.mkv"])
            .unwrap()
            .players()
            .collect::<Vec<_>>();
        assert_eq!(input.len(), 1);
        assert!(input[0].launch.stdin);
        assert_eq!(input[0].files, [OsString::from("-")]);
        let disabled = parse(&["-", "--no-stdin", "real.mkv"])
            .unwrap()
            .players()
            .next()
            .unwrap();
        assert!(!disabled.launch.stdin);
        assert_eq!(disabled.files, [OsString::from("real.mkv")]);
    }

    #[test]
    fn malformed_and_conflicting_options_fail_before_launch() {
        for args in [
            &["--pip", "--music-mode"][..],
            &["--mpv-volume=NaN"],
            &["--mpv-pause=wrong"],
            &["--mpv-wid=123"],
            &["--mpv-"],
        ] {
            assert!(parse(args).is_err());
        }
    }
}
