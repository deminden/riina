"""Control only the ephemeral reference player's documented mpv IPC endpoint."""
import json
from pathlib import Path
import socket
import subprocess
import sys
import time

endpoint, output, fixture = sys.argv[1:]
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

    command("set_property", "pause", True)
    command("seek", 2, "absolute+exact")

    def snapshot():
        return {name: command("get_property", name) for name in
                ["pause", "time-pos", "seeking", "filename", "mpv-version"]}

    while True:
        before = snapshot()
        if (before["pause"] is True and before["seeking"] is False
                and abs(before["time-pos"] - 2) < 0.05
                and before["filename"] == fixture):
            break
        if time.monotonic() >= deadline:
            raise RuntimeError(f"Requested reference state was not reached: {before}")
        time.sleep(0.1)

    # Capture is bracketed by actual engine observations. A launch flag or a
    # successful seek reply alone cannot establish the image's playback state.
    result = subprocess.run(["swift", "scripts/capture-iina-reference.swift", str(output), fixture])
    after = snapshot()
    valid = (after["pause"] is True and after["seeking"] is False
             and abs(after["time-pos"] - 2) < 0.05 and after["filename"] == fixture)
    record = {"schema_version": 1, "before_capture": before, "after_capture": after,
              "playback_state_verified": valid, "capture_exit_code": result.returncode,
              "source": "Observed through official reference mpv JSON IPC; not riina state"}
    (output / "playback-observation.json").write_bytes((json.dumps(record, indent=2) + "\n").encode())
    if result.returncode != 0 or not valid:
        raise RuntimeError("Reference capture or playback-state verification failed")
