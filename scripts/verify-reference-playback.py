"""Control only the ephemeral reference player's documented mpv IPC endpoint."""
import json
from pathlib import Path
import socket
import subprocess
import sys
import time

endpoint, output, fixture, capture_program = sys.argv[1:]
output = Path(output)
deadline = time.monotonic() + 30
connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
connection.settimeout(5)
while True:
    try:
        connection.connect(endpoint)
        break
    except (FileNotFoundError, ConnectionRefusedError):
        if time.monotonic() >= deadline:
            raise RuntimeError("The reference IPC endpoint did not become ready")
        time.sleep(0.1)

with connection, connection.makefile("rwb") as stream:
    request_id = 0

    class ReferenceCommandError(RuntimeError):
        def __init__(self, error):
            self.error = error
            super().__init__(f"Reference command failed: {error}")

    def command(*args):
        global request_id
        request_id += 1
        stream.write((json.dumps({"command": args, "request_id": request_id}) + "\n").encode())
        stream.flush()
        while True:
            line = stream.readline()
            if not line:
                raise RuntimeError("Reference IPC closed before replying")
            reply = json.loads(line)
            if reply.get("request_id") == request_id:
                if reply.get("error") != "success":
                    raise ReferenceCommandError(reply.get("error"))
                return reply.get("data")

    # The socket can be ready before a media file. Wait on media properties,
    # rather than seeking against an idle player or relying on a startup sleep.
    while True:
        try:
            if (command("get_property", "filename") == fixture
                    and isinstance(command("get_property", "time-pos"), (int, float))):
                break
        except ReferenceCommandError as error:
            if error.error != "property unavailable":
                raise
        if time.monotonic() >= deadline:
            raise RuntimeError("The original reference fixture did not load")
        time.sleep(0.1)

    readiness = subprocess.run([capture_program, str(output / "readiness"), fixture, "--wait-window"])
    if readiness.returncode != 0:
        raise RuntimeError("The reference media window did not become visible; see readiness metadata")

    def snapshot():
        return {name: command("get_property", name) for name in
                ["pause", "time-pos", "seeking", "filename", "mpv-version"]}

    def matches_requested_state(state):
        return (state["pause"] is True and state["seeking"] is False
                and isinstance(state["time-pos"], (int, float))
                and abs(state["time-pos"] - 2) < 0.05 and state["filename"] == fixture)

    attempts = []
    for attempt in range(1, 4):
        command("set_property", "pause", True)
        command("seek", 2, "absolute+exact")
        settle_deadline = time.monotonic() + 8
        stable_since = None
        while True:
            before = snapshot()
            if matches_requested_state(before):
                if stable_since is None:
                    stable_since = time.monotonic()
                if time.monotonic() - stable_since >= 0.75:
                    break
            else:
                stable_since = None
                command("set_property", "pause", True)
                command("seek", 2, "absolute+exact")
            if time.monotonic() >= settle_deadline:
                raise RuntimeError(f"Reference playback did not stabilize: {before}")
            time.sleep(0.1)

        # A late IINA startup transition can undo IPC pause. Preserve failed
        # images/observations and retry only after an observed state mismatch.
        attempt_output = output / f"attempt-{attempt:02}"
        result = subprocess.run([capture_program, str(attempt_output), fixture])
        after = snapshot()
        valid = matches_requested_state(after)
        attempts.append({"attempt": attempt, "before_capture": before,
                         "after_capture": after, "playback_state_verified": valid,
                         "capture_exit_code": result.returncode,
                         "capture_directory": attempt_output.name})
        record = {"schema_version": 1, "attempts": attempts,
                  "accepted_attempt": attempt if valid and result.returncode == 0 else None,
                  "source": "Observed through official reference mpv JSON IPC; not riina state"}
        (output / "playback-observation.json").write_bytes((json.dumps(record, indent=2) + "\n").encode())
        if result.returncode != 0:
            raise RuntimeError("Reference window capture failed; see metadata")
        if valid:
            break
    else:
        raise RuntimeError("Reference playback changed during all three bounded captures")
