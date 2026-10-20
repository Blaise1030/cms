#!/usr/bin/env python3
"""Runtime half of update-e2e.sh: drives installed build N (0.0.1) through a busy
deferral and an update to N+1 (0.0.2). Prints PASS/FAIL lines; exits 1 on any FAIL.

Only processes started from the test install path are touched.
"""
import argparse
import base64
import datetime
import http.client
import http.server
import json
import os
import platform
import plistlib
import re
import signal
import socket
import subprocess
import sys
import threading
import time

IDENTIFIER = "com.orso.desktop.updatetest"
AUTO_APPLY_ENV = "ORSO_DESKTOP_UPDATE_E2E_AUTO_APPLY"
# Busy from the Enter; ticks for 30 s, then the daemon's 15 s quiet period ends the turn.
# `sleep 300 &` leaves a PTY descendant that the daemon's shutdown must reap.
AGENT = "sleep 300 & while read x; do i=0; while [ $i -lt 30 ]; do echo tick $i; i=$((i+1)); sleep 1; done; done; wait"
OVERALL_TIMEOUT_S = 420

results = []


def check(name, ok, detail=""):
    results.append(bool(ok))
    print(f"{'PASS' if ok else 'FAIL'}: {name}" + (f" — {detail}" if detail else ""), flush=True)
    return ok


def info(msg):
    print(f"INFO: {msg}", flush=True)


def wait_for(pred, timeout, interval=0.25):
    deadline = time.time() + timeout
    while time.time() < deadline:
        value = pred()
        if value:
            return value
        time.sleep(interval)
    return pred()


def alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    out = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True).stdout
    return bool(out.strip()) and not out.strip().startswith("Z")


def pids_matching(pattern):
    out = subprocess.run(["pgrep", "-f", pattern], capture_output=True, text=True).stdout
    return [int(p) for p in out.split()]


def children(pid):
    out = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True).stdout
    return [int(p) for p in out.split()]


def descendants(pid):
    found, todo = [], [pid]
    while todo:
        for child in children(todo.pop()):
            found.append(child)
            todo.append(child)
    return found


def command(pid):
    return subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def api(base_url, token, method, path, body=None):
    port = int(base_url.rsplit(":", 1)[1])
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    conn.request(method, path, json.dumps(body) if body is not None else None, headers)
    resp = conn.getresponse()
    data = resp.read()
    conn.close()
    try:
        return resp.status, json.loads(data)
    except ValueError:
        return resp.status, data.decode(errors="replace")


def healthy(base_url, token):
    try:
        status, body = api(base_url, token, "GET", "/api/health")
        return status == 200 and isinstance(body, dict) and body.get("status") == "ok"
    except OSError:
        return False


def send_enter(base_url, token, project):
    """Minimal WebSocket client: connect to the PTY stream and send a lone Enter."""
    port = int(base_url.rsplit(":", 1)[1])
    sock = socket.create_connection(("127.0.0.1", port), timeout=5)
    key = base64.b64encode(os.urandom(16)).decode()
    sock.sendall(
        (
            f"GET /api/terminal/{project}/ws?t={token} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n"
            f"Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        ).encode()
    )
    head = b""
    while b"\r\n\r\n" not in head:
        chunk = sock.recv(4096)
        if not chunk:
            break
        head += chunk
    if not head.startswith(b"HTTP/1.1 101"):
        sock.close()
        return head.split(b"\r\n", 1)[0].decode(errors="replace")

    def frame(opcode, payload):
        mask = os.urandom(4)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        return bytes([0x80 | opcode, 0x80 | len(payload)]) + mask + masked

    sock.sendall(frame(0x2, b"\r"))
    time.sleep(0.5)
    sock.sendall(frame(0x8, b""))
    sock.close()
    return "ok"


def plist_version(app):
    with open(os.path.join(app, "Contents", "Info.plist"), "rb") as f:
        return plistlib.load(f).get("CFBundleShortVersionString")


class Log:
    def __init__(self, path):
        self.path = path

    def text(self):
        try:
            with open(self.path, errors="replace") as f:
                return f.read()
        except FileNotFoundError:
            return ""

    def has(self, needle):
        return needle in self.text()

    def count(self, needle):
        return self.text().count(needle)


def serve(directory, port, manifest, release):
    """Serves the N+1 artifacts. latest.json is held back until `release` is set,
    so N's launch-time check only sees the update once the session is busy."""
    hits = []

    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **kw):
            super().__init__(*a, directory=directory, **kw)

        def log_message(self, fmt, *a):
            hits.append(self.path)

        def do_GET(self):
            if self.path.split("?")[0] == "/latest.json":
                release.wait(timeout=180)
                body = json.dumps(manifest).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            super().do_GET()

    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, hits


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", required=True)
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()

    signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError("overall timeout")))
    signal.alarm(OVERALL_TIMEOUT_S)

    work = args.work
    app = os.path.join(work, "Applications", "orso.app")
    exe = os.path.join(app, "Contents", "MacOS", "orso-desktop")
    home = os.path.join(work, "home")
    log = Log(os.path.join(work, "app.log"))
    daemon_file = os.path.join(home, "update-e2e-daemon.json")
    subprocess.run(["rm", "-rf", home, log.path])

    projects = os.path.join(home, "orso-projects")
    os.makedirs(os.path.join(projects, ".orso"))
    os.makedirs(os.path.join(projects, "site"))
    os.makedirs(os.path.join(home, ".config", "orso"))
    with open(os.path.join(projects, ".orso", "projects.json"), "w") as f:
        json.dump([{"id": "site", "name": "Site", "port": free_port()}], f)
    with open(os.path.join(home, ".config", "orso", "agents.json"), "w") as f:
        json.dump({"fake": ["sh", "-c", AGENT]}, f)

    arch = "aarch64" if platform.machine() == "arm64" else "x86_64"
    tarball = "orso-0.0.2.app.tar.gz"
    with open(os.path.join(work, "serve", tarball + ".sig")) as f:
        signature = f.read().strip()
    manifest = {
        "version": "0.0.2",
        "notes": "update-e2e",
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {f"darwin-{arch}": {"signature": signature, "url": f"http://127.0.0.1:{args.port}/{tarball}"}},
    }
    release = threading.Event()
    server, hits = serve(os.path.join(work, "serve"), args.port, manifest, release)
    info(f"serving N+1 on 127.0.0.1:{args.port}")

    check("installed N is 0.0.1 (Info.plist)", plist_version(app) == "0.0.1", plist_version(app))
    n = None
    pty = []
    try:
        env = dict(os.environ, **{AUTO_APPLY_ENV: "1"})
        n = subprocess.Popen(
            [exe, "--update-e2e-home", home, "--update-e2e-log", log.path],
            env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        info(f"N app pid={n.pid}")

        daemon = wait_for(lambda: os.path.exists(daemon_file) and json.load(open(daemon_file)), 40)
        if not check("N started and published its daemon", daemon, log.path):
            return
        base_url, token = daemon["base_url"], daemon["token"]
        check("N daemon healthy", wait_for(lambda: healthy(base_url, token), 20), base_url)
        check("N reports version 0.0.1", log.has("update-e2e: running version 0.0.1"))
        n_daemons = [p for p in children(n.pid) if "serve --port 0" in command(p)]
        check("N has exactly one `serve --port 0` child", len(n_daemons) == 1, str(n_daemons))
        n_daemon = n_daemons[0] if n_daemons else None
        info(f"N daemon pid={n_daemon}")

        status, body = api(base_url, token, "POST", "/api/terminal", {"project": "site", "provider": "fake"})
        if not check("terminal session started", status == 200 and isinstance(body, dict) and body.get("running"), f"{status} {body}"):
            return
        pty = wait_for(
            lambda: [p for p in descendants(n_daemon) if "sleep 300" in command(p)] and descendants(n_daemon), 15
        ) or []
        info("PTY descendants: " + ", ".join(f"{p} ({command(p)[:60]})" for p in pty or []))
        check("PTY descendants running", pty)

        check("sent a lone Enter over the PTY WebSocket", send_enter(base_url, token, "site") == "ok")
        busy = wait_for(lambda: api(base_url, token, "GET", "/api/terminal/site")[1].get("busy"), 10)
        if not check("terminal session is busy", busy):
            return

        release.set()
        check("updater found 0.0.2", wait_for(lambda: log.has("downloading update 0.0.2"), 60))
        check("update 0.0.2 downloaded (signature verified on install)", wait_for(lambda: log.has("update 0.0.2 downloaded"), 60))

        refused = wait_for(lambda: log.has("update-e2e: apply refused: A terminal session is still working"), 20)
        check("apply refused while busy", refused)
        time.sleep(8)
        still_busy = api(base_url, token, "GET", "/api/terminal/site")[1].get("busy")
        check(
            "deferred while busy: N, its daemon and the 0.0.1 install untouched",
            refused and still_busy and n.poll() is None and alive(n_daemon)
            and plist_version(app) == "0.0.1" and not log.has("installing update"),
            f"busy={still_busy}",
        )

        def idle_or_stopped():
            # N applies as soon as the turn is over, so its daemon may already be gone.
            try:
                return not api(base_url, token, "GET", "/api/terminal/site")[1].get("busy")
            except OSError:
                return True

        check("session went idle (turn over)", wait_for(idle_or_stopped, 90, 1))
        exited = wait_for(lambda: n.poll() is not None, 60)
        check("N applied once idle and exited", exited, f"exit code {n.poll()}")
        check("N stopped its daemon before installing", log.has("installing update 0.0.2"))
        check("N daemon exited 0", log.count("daemon exited (code Some(0), signal None)") >= 1 and not alive(n_daemon))
        left = [p for p in pty or [] if alive(p)]
        check("N's PTY descendants are gone", not left, str(left))
        check("install replaced the app on disk (Info.plist 0.0.2)", plist_version(app) == "0.0.2", plist_version(app))

        def relaunched():
            return [p for p in pids_matching(re.escape(exe)) if p != n.pid]

        new = wait_for(relaunched, 60)
        if not check("N+1 relaunched from the same path", new, str(new)):
            return
        new_pid = new[0]
        info(f"N+1 app pid={new_pid}")
        check("N+1 reports version 0.0.2", wait_for(lambda: log.has("update-e2e: running version 0.0.2"), 30))
        daemon2 = wait_for(
            lambda: (d := json.load(open(daemon_file))) and d["base_url"] != base_url and d, 40
        )
        check("N+1 published a new daemon", daemon2)
        if daemon2:
            check("N+1 daemon healthy", wait_for(lambda: healthy(daemon2["base_url"], daemon2["token"]), 20), daemon2["base_url"])
        new_daemons = [p for p in children(new_pid) if "serve --port 0" in command(p)]
        check("N+1 has exactly one `serve --port 0` child", len(new_daemons) == 1, str(new_daemons))
        check("N+1's check finds no newer update", wait_for(lambda: log.has("update check: orso is up to date"), 30))

        subprocess.run(["osascript", "-e", f'tell application id "{IDENTIFIER}" to quit'], capture_output=True, timeout=15)
        check("N+1 quit", wait_for(lambda: not alive(new_pid), 20))
        check("N+1 daemon exited 0", wait_for(lambda: log.count("daemon exited (code Some(0), signal None)") >= 2, 10))
        info(f"latest.json/artifact requests: {hits}")
    except Exception as e:  # any harness error is a FAIL
        check("harness ran without errors", False, repr(e))
    finally:
        signal.alarm(0)
        server.shutdown()
        leftovers = pids_matching(re.escape(os.path.join(work, "Applications"))) + [p for p in pty if alive(p)]
        for pid in leftovers:
            os.kill(pid, signal.SIGTERM)
        if leftovers:
            time.sleep(3)
            for pid in pids_matching(re.escape(os.path.join(work, "Applications"))) + [p for p in pty if alive(p)]:
                os.kill(pid, signal.SIGKILL)
        if n and n.poll() is None:
            n.kill()
        check("no processes left from the test install", not leftovers, str(leftovers))


if __name__ == "__main__":
    main()
    print(f"\n{sum(results)}/{len(results)} checks passed", flush=True)
    sys.exit(0 if results and all(results) else 1)
