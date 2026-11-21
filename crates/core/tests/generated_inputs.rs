//! Generated cases for portable input parsing and geometry invariants.

use proptest::{collection, prelude::*, sample::select};
use riina_core::{
    Preferences,
    crop::{Rectangle, VideoFilter},
    crop_geometry::{DisplayRect, Point, Projection},
    key_bindings::{self, ActionKind, Mapping},
    plugin::{Manifest, network_url_allowed},
    subtitle_color::SubtitleColor,
    time_input::parse_video_time,
    video_controls::Rotation,
    video_filters::{format_filter, is_active, valid_filter_string},
    window_aspect::PixelSize,
    window_resize::{self, ResizeOption},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

fn text() -> impl Strategy<Value = String> {
    collection::vec(any::<char>(), 0..96).prop_map(|characters| characters.into_iter().collect())
}

fn arbitrary_json() -> impl Strategy<Value = Value> {
    let scalar = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|value| json!(value)),
        text().prop_map(Value::String),
    ];
    scalar.prop_recursive(3, 40, 6, |value| {
        let key = prop_oneof![
            select(vec![
                "volume",
                "volume_boost",
                "schema_version",
                "network",
                "allowedDomains"
            ])
            .prop_map(str::to_owned),
            "[a-z_]{1,16}",
        ];
        prop_oneof![
            collection::vec(value.clone(), 0..6).prop_map(Value::Array),
            collection::btree_map(key, value, 0..6)
                .prop_map(|values| Value::Object(values.into_iter().collect())),
        ]
    })
}

fn mapping() -> impl Strategy<Value = Mapping> {
    (
        "[A-Z0-9+]{1,12}",
        "[a-z]+( [0-9]{1,4})?",
        any::<bool>(),
        proptest::option::of("[A-Za-z0-9]{1,16}"),
    )
        .prop_map(|(key, action, riina, comment)| Mapping {
            key,
            action,
            kind: if riina {
                ActionKind::Riina
            } else {
                ActionKind::Mpv
            },
            comment,
        })
}

proptest! {
    #[test]
    fn arbitrary_text_is_handled_by_input_parsers(input in text()) {
        let _ = key_bindings::parse(&input);
        let _ = Manifest::parse(&input);
        if let Some(seconds) = parse_video_time(&input) {
            prop_assert!(seconds.is_finite() && seconds >= 0.0);
        }
        if valid_filter_string(&input) {
            prop_assert!(!input.trim().is_empty());
            prop_assert!(!input.chars().any(char::is_control));
            prop_assert!(input.len() <= 4096);
        }
        let (recovered, _) = Preferences::parse_lenient(&input);
        prop_assert!(Preferences::parse(&serde_json::to_string(&recovered).unwrap()).is_ok());
    }

    #[test]
    fn input_profiles_round_trip(entries in collection::vec(mapping(), 0..24)) {
        let encoded = key_bindings::serialize(&entries).unwrap();
        let decoded = key_bindings::parse(&encoded).unwrap();
        prop_assert_eq!(decoded, entries);
    }

    #[test]
    fn hours_minutes_and_fractional_seconds_parse_arithmetically(
        hours in 0_u32..24_000,
        minutes in 0_u32..120,
        ticks in 0_u32..1_200,
    ) {
        let seconds = f64::from(ticks) / 8.0;
        let expected = f64::from(hours) * 3600.0 + f64::from(minutes) * 60.0 + seconds;
        prop_assert_eq!(parse_video_time(&format!("{hours}:{minutes}:{seconds}")), Some(expected));
    }

    #[test]
    fn mpv_subtitle_color_round_trips(channels in any::<[u8; 4]>()) {
        let color = SubtitleColor::from_rgba(channels.map(|value| f64::from(value) / 255.0)).unwrap();
        prop_assert_eq!(SubtitleColor::from_mpv(&color.mpv_value()), Some(color));
        let [r, g, b, a] = channels;
        prop_assert_eq!(SubtitleColor::from_mpv(&format!("#{a:02x}{r:02x}{g:02x}{b:02x}")), Some(color));
    }

    #[test]
    fn formatted_filters_preserve_identity(
        label in proptest::option::of("[a-z]{1,8}"),
        values in collection::vec(0_i32..4096, 0..10),
    ) {
        let filter = VideoFilter {
            name: "crop".into(),
            label,
            enabled: Some(true),
            params: values.iter().enumerate().map(|(index, value)| {
                (format!("@{index}"), value.to_string())
            }).collect::<BTreeMap<_, _>>(),
        };
        let formatted = format_filter(&filter);
        prop_assert!(valid_filter_string(&formatted));
        prop_assert!(is_active(&formatted, &[filter]));
        prop_assert!(!valid_filter_string(&(formatted + "\n")));
    }

    #[test]
    fn window_fit_stays_in_the_work_area_and_preserves_aspect(
        video_width in 240_u32..4097,
        video_height in 180_u32..2161,
        width in 500_u32..3201,
        height in 450_u32..2001,
        rotated in any::<bool>(),
    ) {
        let display = PixelSize { width, height };
        let target = window_resize::target_size(
            f64::from(video_width),
            f64::from(video_height),
            Some(if rotated { 90 } else { 0 }),
            1.0,
            window_resize::Settings { option: ResizeOption::FitScreen, ..Default::default() },
            Some(display),
            PixelSize { width: 1, height: 1 },
        ).unwrap();
        prop_assert!(target.width <= width && target.height <= height);
        let (w, h) = if rotated { (video_height, video_width) } else { (video_width, video_height) };
        let cross = i64::from(target.width) * i64::from(h) - i64::from(target.height) * i64::from(w);
        prop_assert!(cross.abs() <= i64::from(w.max(h)), "The fitted size must round by at most one pixel");
    }

    #[test]
    fn crop_projection_round_trips_and_snaps_at_each_rotation(
        width in 80_u32..4097,
        height in 80_u32..2161,
        x in 0_u32..100,
        y in 0_u32..100,
        extent in 10_u32..100,
        degrees in select(vec![0_i64, 90, 180, 270]),
    ) {
        let x = width * x / 100;
        let y = height * y / 100;
        let rectangle = Rectangle {
            x, y,
            width: ((width - x) * extent / 100).max(1),
            height: ((height - y) * extent / 100).max(1),
        };
        let projection = Projection::contain(
            (width, height),
            (f64::from(width), f64::from(height)),
            Rotation::from_degrees(degrees).unwrap(),
            DisplayRect { x: 12.0, y: 28.0, width: 840.0, height: 450.0 },
        ).unwrap();
        let shown = projection.project(rectangle).unwrap();
        let selected = projection.select_continuous(
            Point { x: shown.x, y: shown.y },
            Point { x: shown.x + shown.width, y: shown.y + shown.height },
        ).unwrap();
        for (actual, source, size) in [
            (selected.x, rectangle.x, width),
            (selected.y, rectangle.y, height),
            (selected.x + selected.width, rectangle.x + rectangle.width, width),
            (selected.y + selected.height, rectangle.y + rectangle.height, height),
        ] {
            prop_assert!(actual >= 0.0 && actual <= f64::from(size));
            let edge = if source <= 4 { 0 } else { size };
            let near_edge = source.min(size - source);
            let within_rounding = |expected| (actual - f64::from(expected)).abs() < 1e-6;
            if near_edge == 4 {
                // Projection rounding can put the exact threshold on either side of the snap.
                prop_assert!(within_rounding(source) || within_rounding(edge));
            } else {
                let expected = if near_edge < 4 { edge } else { source };
                prop_assert!(within_rounding(expected));
            }
        }
    }

    #[test]
    fn validated_preferences_round_trip(
        volume in 0_u16..=800,
        boost in any::<bool>(),
        animations in any::<bool>(),
        threads in 0_u32..32,
    ) {
        let prefs = Preferences {
            volume: f64::from(volume.min(if boost { 800 } else { 400 })) / 4.0,
            volume_boost: boost,
            disable_animations: animations,
            video_threads: threads,
            ..Preferences::default()
        };
        let first = serde_json::to_vec(&prefs).unwrap();
        let decoded = Preferences::parse(std::str::from_utf8(&first).unwrap()).unwrap();
        prop_assert_eq!(serde_json::to_vec(&decoded).unwrap(), first);
    }

    #[test]
    fn recovered_json_always_has_a_strict_serialization(value in arbitrary_json()) {
        let (recovered, _) = Preferences::parse_lenient(&value.to_string());
        prop_assert!(Preferences::parse(&serde_json::to_string(&recovered).unwrap()).is_ok());
    }

    #[test]
    fn valid_plugin_manifests_keep_the_declared_entry_and_domains(
        id in "[a-z]{1,12}",
        name in "[A-Za-z0-9 ]{1,16}",
        entry in "[a-z]{1,12}",
        host in "[a-z]{1,12}",
    ) {
        prop_assume!(!name.trim().is_empty());
        let entry = format!("scripts/{entry}.js");
        let domain = format!("{host}.example.com");
        let document = json!({
            "identifier": format!("com.example.{id}"),
            "version": "1.0",
            "name": name,
            "entry": entry,
            "author": {"name": "Test fixture"},
            "allowedDomains": [domain]
        });
        let manifest = Manifest::parse(&document.to_string()).unwrap();
        prop_assert_eq!(manifest.name, name.trim());
        prop_assert_eq!(manifest.entry, Path::new(&entry));
        prop_assert_eq!(manifest.allowed_domains, [domain]);
    }

    #[test]
    fn allow_lists_distinguish_the_configured_domain_and_its_subdomains(
        host in "[a-z]{1,12}",
        child in "[a-z]{1,12}",
        wildcard in any::<bool>(),
    ) {
        let domain = format!("{host}.example.com");
        let allowed = if wildcard { format!("*.{domain}") } else { domain.clone() };
        let allowed = [allowed];
        prop_assert_eq!(network_url_allowed(&format!("https://{domain}/asset"), &allowed), !wildcard);
        prop_assert_eq!(network_url_allowed(&format!("https://{child}.{domain}/asset"), &allowed), wildcard);
        let other_host = format!("https://{child}.example.net/asset");
        let file = format!("file://{domain}/asset");
        prop_assert!(!network_url_allowed(&other_host, &allowed));
        prop_assert!(!network_url_allowed(&file, &allowed));
    }
}
