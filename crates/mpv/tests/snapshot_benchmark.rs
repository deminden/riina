//! Measure only snapshot publication and handoff. No decoder or GUI is started.
use riina_core::{MAX_RETAINED_MPV_EVENTS, Playback, PlaylistEntry};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: each operation delegates to System using the original layout, pointer and size.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the caller fulfils GlobalAlloc's allocation contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the caller fulfils GlobalAlloc's allocation contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the pointer and original layout came from System.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and original layout came from System.
        unsafe { System.dealloc(pointer, layout) };
    }
}

fn state() -> Result<Playback, serde_json::Error> {
    let tracks = (0..30)
        .map(|id| {
            serde_json::from_value(json!({
                "id": id + 1, "type": "audio", "title": format!("Track {id}"),
                "lang": "eng", "codec": "aac", "selected": id == 0,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let playlist = (0..500)
        .map(|id| PlaylistEntry {
            id,
            filename: format!("fixtures/fixture-{id:03}.mkv"),
            title: Some(format!("Fixture {id}")),
            current: id == 0,
            playing: id == 0,
        })
        .collect();
    let mut state = Playback {
        tracks,
        playlist,
        ..Default::default()
    };
    for _ in 0..MAX_RETAINED_MPV_EVENTS {
        state.record_mpv_event("property-change".into());
    }
    Ok(state)
}

#[test]
#[ignore = "Measures allocation and time for 10,000 published snapshots"]
fn publish_time_position_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    const UPDATES: u32 = 10_000;
    let mut state = state()?;
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let start = Instant::now();
    for i in 0..UPDATES {
        state.position = Some(f64::from(i) / 100.0);
        let snapshot = black_box(state.clone());
        // The observer retains a second copy as the latest UI state.
        let retained = black_box(snapshot.clone());
        black_box((snapshot, retained));
    }
    let elapsed = start.elapsed();
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;
    writeln!(
        std::io::stdout().lock(),
        "{UPDATES} updates, {allocations} allocations, {:.3} ms",
        elapsed.as_secs_f64() * 1_000.0
    )?;
    Ok(())
}
