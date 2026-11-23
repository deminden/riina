//! Put plugin writes through the granted capabilities before they reach libmpv.
use super::AvailablePermissions;

pub(super) enum Scope {
    Playback,
    FileList,
}

pub(super) fn scope(name: &str, permissions: AvailablePermissions) -> Result<Scope, String> {
    match name {
        "volume-max" => Err("mpv.set: change the volume range in Riina Preferences".into()),
        "audio-files" | "external-files" | "sub-files" => permissions
            .file_system
            .then_some(Scope::FileList)
            .ok_or_else(|| format!("mpv.set: {name} requires the file-system permission")),
        "cookies" | "hls-bitrate" | "http-header-fields" | "network-timeout" | "referrer"
        | "rtsp-transport" | "tls-verify" | "user-agent" => permissions
            .network_request
            .then_some(Scope::Playback)
            .ok_or_else(|| format!("mpv.set: {name} requires the network-request permission")),
        _ if PLAYBACK_PROPERTIES
            .split_ascii_whitespace()
            .any(|property| property == name) =>
        {
            Ok(Scope::Playback)
        }
        _ => Err(format!(
            "mpv.set: {name} is outside the plugin property capabilities"
        )),
    }
}

// Public mpv option names from the IINA v1.5.0 playback capability inventory.
// File and network settings and Riina's volume range are handled above.
const PLAYBACK_PROPERTIES: &str = "
ab-loop-a ab-loop-b ab-loop-count ad-lavc-ac3drc ad-lavc-downmix ad-lavc-threads aid alang
audio-buffer audio-channels audio-delay audio-display audio-exclusive audio-format
audio-normalize-downmix audio-pitch-correction audio-resample-cutoff audio-resample-filter-size
audio-resample-linear audio-resample-max-output-size audio-resample-phase-shift audio-samplerate
audio-set-media-role audio-spdif audio-stream-silence audio-wait-open brightness cache
cache-pause cache-pause-initial cache-pause-wait cache-secs container-fps-override contrast
correct-pts deinterlace deinterlace-field-parity demuxer-lavf-analyzeduration
demuxer-lavf-probescore demuxer-lavf-probesize demuxer-max-back-bytes demuxer-max-bytes
demuxer-mkv-subtitle-preroll demuxer-mkv-subtitle-preroll-secs demuxer-readahead-secs
display-fps-override edition embeddedfonts end force-seekable framedrop frames fullscreen gamma
gapless-audio geometry hr-seek hr-seek-framedrop hue hwdec hwdec-codecs hwdec-extra-frames
hwdec-image-format hwdec-software-fallback hwdec-threads image-display-duration
image-subs-hdr-peak image-subs-video-resolution initial-audio-sync interpolation keep-open
keep-open-pause length loop-file loop-playlist mute ontop panscan pause pitch play-direction
rebase-start-time replaygain replaygain-clip replaygain-fallback replaygain-preamp saturation
secondary-sid secondary-sub-ass-override secondary-sub-delay secondary-sub-pos
secondary-sub-visibility sid slang speed sstep start stop-screensaver stretch-dvd-subs
stretch-image-subs-to-screen sub-align-x sub-align-y sub-ass sub-ass-force-margins
sub-ass-justify sub-ass-override sub-ass-prune-delay sub-ass-scale-with-window
sub-ass-style-overrides sub-ass-use-video-data sub-ass-video-aspect-override
sub-ass-vsfilter-color-compat sub-back-color sub-blur sub-bold sub-border-style sub-clear-on-seek
sub-codepage sub-color sub-create-cc-track sub-delay sub-filter-sdh sub-filter-sdh-enclosures
sub-filter-sdh-harder sub-fix-timing sub-fix-timing-keep sub-fix-timing-threshold sub-font
sub-font-provider sub-font-size sub-forced-events-only sub-fps sub-gauss sub-gray sub-hdr-peak
sub-hinting sub-italic sub-justify sub-line-spacing sub-margin-x sub-margin-y sub-outline-color
sub-outline-size sub-past-video-end sub-pos sub-scale sub-scale-by-window sub-scale-signs
sub-scale-with-window sub-shadow-offset sub-shaper sub-spacing sub-speed sub-stretch-durations
sub-use-margins sub-visibility sub-vsfilter-bidi-compat subs-fallback subs-fallback-forced
subs-match-os-language subs-with-matching-audio swapchain-depth teletext-page
track-auto-selection vd-apply-cropping vd-lavc-check-hw-profile vd-lavc-dr vd-lavc-fast
vd-lavc-film-grain vd-lavc-framedrop vd-lavc-skipframe vd-lavc-skipidct vd-lavc-skiploopfilter
vd-lavc-threads vid video-align-x video-align-y video-aspect-method video-aspect-override
video-crop video-latency-hacks video-margin-ratio-bottom video-margin-ratio-left
video-margin-ratio-right video-margin-ratio-top video-output-levels video-pan-x video-pan-y
video-recenter video-rotate video-scale-x video-scale-y video-sync video-unscaled video-zoom
vlang volume volume-gain volume-gain-max volume-gain-min
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_settings_network_options_and_file_lists_use_separate_capabilities() {
        let empty = AvailablePermissions::default();
        assert!(matches!(scope("speed", empty), Ok(Scope::Playback)));
        assert!(scope("sub-files", empty).is_err());
        assert!(scope("http-header-fields", empty).is_err());
        let files = AvailablePermissions {
            file_system: true,
            ..empty
        };
        assert!(matches!(scope("sub-files", files), Ok(Scope::FileList)));
        assert!(scope("http-header-fields", files).is_err());
        let both = AvailablePermissions {
            network_request: true,
            ..files
        };
        assert!(matches!(
            scope("http-header-fields", both),
            Ok(Scope::Playback)
        ));
        for property in [
            "volume-max",
            "options/volume-max",
            "input-ipc-server",
            "scripts",
        ] {
            assert!(
                scope(property, both).is_err(),
                "{property} must stay under app control"
            );
        }
    }
}
