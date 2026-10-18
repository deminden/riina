"""Validate evidence from the exact extracted application process."""
import argparse
import json
from pathlib import Path

CHECKS = {
    "preference_writers_preserve_settings",
    "playing_native_video_and_audio_menu",
    "volume_preserves_surfaces",
    "double_click_fullscreen",
    "controller_reveal_keeps_native_surface",
    "fullscreen_exit_and_playback_progress",
}


def validate(report, executable, process_id, fixture):
    if not isinstance(report, dict):
        raise ValueError("Smoke report must be an object")
    if report.get("success") is not True:
        raise ValueError(f"Application smoke failed: {report.get('error')}")
    if "error" not in report or report["error"] is not None:
        raise ValueError("Successful smoke report must have no error")
    if (type(process_id) is not int or process_id <= 0
            or type(report.get("pid")) is not int or report["pid"] != process_id):
        raise ValueError("Smoke report belongs to another process")
    actual = report.get("executable")
    if not isinstance(actual, str) or Path(actual).resolve() != Path(executable).resolve():
        raise ValueError("Smoke report does not identify the extracted executable")
    if report.get("title") != f"{Path(fixture).name} \u2014 riina":
        raise ValueError("Smoke did not open the selected isolated fixture")
    elapsed = report.get("elapsed_ms")
    if type(elapsed) is not int or not 0 < elapsed <= 40_000:
        raise ValueError("Smoke report has no valid duration within its deadline")
    checks = report.get("checks")
    if not isinstance(checks, list) or any(not isinstance(c, str) for c in checks):
        raise ValueError("Smoke checks must be a list of names")
    if len(checks) != len(CHECKS) or set(checks) != CHECKS:
        raise ValueError("Application smoke omitted or duplicated required checks")
    if report.get("physical_display_verified") is not False:
        raise ValueError("Automated smoke cannot certify physical-display presentation")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--fixture", required=True, type=Path)
    args = parser.parse_args()
    validate(json.loads(args.report.read_text(encoding="utf-8")), args.executable, args.pid, args.fixture)
    print("Extracted-application playback, controls and shutdown evidence accepted")


if __name__ == "__main__":
    main()
