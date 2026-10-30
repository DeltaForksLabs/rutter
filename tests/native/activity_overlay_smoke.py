"""Real activity host/producer smoke on an owned 1100x800, 1x compositor.

Build beforehand, sequentially (this driver never builds):
  cargo build --locked --offline --features activity-ipc --example dynamic_island_host
  cargo build --locked --offline --example dynamic_island_probe
Run:
  python3 tests/native/activity_overlay_smoke.py --labwc /path/to/labwc --xvfb /path/to/Xvfb --keep
Alternatively use --fraym /path/to/fraym. Private libraries: LD_LIBRARY_PATH.
User expansion requests bounded Exclusive focus; real Escape must collapse the
host and restore OnDemand. Delivery to the underlying probe is a failure.
No manual visual or other-scale/theme/platform acceptance. X11 rejection is tested
on the same owned display, without connecting to the nested Wayland compositor.
"""

import argparse
import errno
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layer_shell_smoke import ctl, host_window, stop
from native_popup_smoke import PopupInput
from dynamic_island_smoke import screenshot


COMPACT = (90, 8, 240, 76)
EXPANDED = (8, 8, 404, 284)
MAX_FRAME = 16 * 1024


def require(condition, message):
    # Acceptance checks must also work under python -O.
    if not condition:
        raise RuntimeError(message)


def transfer(state="Running"):
    return {"Transfer": {"title": "T", "supporting": "", "direction": "Download",
                         "completed": 1, "total": 4, "state": state}}


class Producer:
    """Serialized stdlib wire peer; the driver pumps Poll while waiting for UI."""

    def __init__(self, path):
        self.connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.connection.settimeout(3)
        self.actions = []
        self.last_poll = 0
        try:
            self.connection.connect(str(path))
            require(self.exchange({"Hello": {"version": 1}}) == "Ready", "Hello not Ready")
        except BaseException:
            self.close()
            raise

    def read_exact(self, count, deadline):
        chunks = bytearray()
        while len(chunks) < count:
            remaining = deadline - time.monotonic()
            require(remaining > 0, "Activity reply exceeded three-second exchange deadline")
            self.connection.settimeout(remaining)
            chunk = self.connection.recv(count - len(chunks))
            require(chunk, "Activity peer disconnected during frame")
            chunks.extend(chunk)
        return bytes(chunks)

    def exchange(self, request):
        payload = json.dumps(request, separators=(",", ":")).encode("utf-8")
        require(0 < len(payload) <= MAX_FRAME, "Outgoing frame outside 1..16384 bytes")
        # One deadline covers the whole exchange, including fragmented responses.
        deadline = time.monotonic() + 3
        try:
            self.connection.sendall(struct.pack("!I", len(payload)) + payload)
            header = self.read_exact(4, deadline)
            length = struct.unpack("!I", header)[0]
            require(0 < length <= MAX_FRAME, f"Invalid incoming frame length: {length}")
            reply = json.loads(self.read_exact(length, deadline))
        finally:
            self.connection.settimeout(3)
        require(not isinstance(reply, dict) or "Error" not in reply, f"Producer rejected: {reply}")
        return reply

    def start(self):
        reply = self.exchange({"Start": transfer()})
        require(isinstance(reply, dict) and type(reply.get("Started")) is int,
                f"Missing Started handle: {reply}")
        self.activity = reply["Started"]

    def replace(self, state):
        reply = self.exchange({"Replace": {"id": self.activity, "content": transfer(state)}})
        require(reply == "Accepted", f"Replace not Accepted: {reply}")

    def pump(self):
        if time.monotonic() - self.last_poll < 0.25:
            return
        reply = self.exchange("Poll")
        require(isinstance(reply, dict) and set(reply) == {"Action"}, f"Invalid Poll reply: {reply}")
        if reply["Action"] is not None:
            self.actions.append(reply["Action"])
        self.last_poll = time.monotonic()

    def close(self):
        self.connection.close()

    def shutdown_eof(self, evidence):
        # serve() may report its pending read timeout before dropping the stream
        # when shutdown stops our Poll heartbeats. Buffered data is not a live
        # connection: validate that one terminal frame, then still require EOF.
        deadline = time.monotonic() + 3
        buffered = bytearray()
        while True:
            remaining = deadline - time.monotonic()
            require(remaining > 0, "Shutdown did not disconnect the live producer")
            self.connection.settimeout(remaining)
            chunk = self.connection.recv(4096)
            if not chunk:
                break
            buffered.extend(chunk)
            require(len(buffered) <= MAX_FRAME + 4, "Excess data before shutdown EOF")
        evidence.write_bytes(buffered)
        if not buffered:
            return None
        require(len(buffered) >= 4, "Truncated terminal reply before shutdown EOF")
        length = struct.unpack("!I", buffered[:4])[0]
        require(0 < length <= MAX_FRAME and len(buffered) == length + 4,
                "Invalid/multiple terminal frames before shutdown EOF")
        reply = json.loads(buffered[4:])
        expected = {"IPC frame deadline elapsed"} | {
            f"{os.strerror(code)} (os error {code})" for code in (errno.EAGAIN, errno.ETIMEDOUT)
        }
        require(isinstance(reply, dict) and set(reply) == {"Error"}
                and reply["Error"] in expected, f"Unexpected terminal shutdown reply: {reply}")
        return reply


def native_surface(trace):
    """Select the rendered layer lifetime, allowing retired renderer probes."""
    roles = list(re.finditer(
        r'get_layer_surface\(new id zwlr_layer_surface_v1[#@](\d+), '
        r'wl_surface[#@](\d+), [^,]+, 3, "rutter-activity-host"\)', trace))
    for match in reversed(roles):
        role, surface = match.groups()
        lifetime = trace[match.end():]
        if re.search(rf"wl_surface[#@]{surface}\.attach\(", lifetime):
            return role, surface, lifetime
    return None


def input_rectangles(trace, surface):
    # Snapshot each set_input_region before wl_region.destroy/reuse. Regions are
    # copied by Wayland; inspecting only the last region object would be wrong.
    regions, latest = {}, None
    pattern = (r"create_region\(new id wl_region[#@](\d+)\)|"
               r"wl_region[#@](\d+)\.add\((-?\d+), (-?\d+), (-?\d+), (-?\d+)\)|"
               rf"wl_surface[#@]{surface}\.set_input_region\((?:wl_region[#@](\d+)|(nil))\)")
    for match in re.finditer(pattern, trace):
        created, region, x, y, width, height, assigned, full = match.groups()
        if created:
            regions[created] = []
        elif region:
            regions.setdefault(region, []).append(tuple(map(int, (x, y, width, height))))
        elif full:
            latest = None
        else:
            latest = list(regions.get(assigned, []))
    return latest


def region_matches(trace, bounds):
    native = native_surface(trace)
    if native is None:
        return False
    rectangles = input_rectangles(trace, native[1])
    if not rectangles:
        return False
    rectangles = [r for r in rectangles if r[2] > 0 and r[3] > 0]
    if not rectangles:
        return False
    left = min(r[0] for r in rectangles)
    top = min(r[1] for r in rectangles)
    right = max(r[0] + r[2] for r in rectangles)
    bottom = max(r[1] + r[3] for r in rectangles)
    if (left, top, right - left, bottom - top) != bounds:
        return False

    def contains(x, y):
        return any(rx <= x < rx + w and ry <= y < ry + h for rx, ry, w, h in rectangles)

    return contains(left + bounds[2] // 2, top + 4) and not contains(left + 1, top + 1)


class Smoke:
    def __init__(self, root, env, args, children, host):
        self.root, self.env, self.args = root, env, args
        self.children, self.host = children, host
        self.client = None
        self.producer = None
        self.clicks = 0
        self.checks, self.limitations = [], []
        self.pixels = {}

    def passed(self, description):
        self.checks.append(description)
        print(f"PASS: {description}", flush=True)

    def spawn(self, command, name, env=None):
        with (self.root / name).open("w") as log:
            child = subprocess.Popen(command, env=self.env if env is None else env,
                                     stdout=log, stderr=subprocess.STDOUT)
        self.children.append(child)
        return child

    def trace(self):
        return self.log.read_text(errors="replace")

    def probe_trace(self):
        require(self.probe.poll() is None, "Underlying probe exited; see passive.log")
        return (self.root / "passive.log").read_text(errors="replace")

    def wait(self, predicate, description, timeout=10, alive=True):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if alive and self.client is not None:
                require(self.client.poll() is None,
                        f"Activity host exited ({self.client.returncode}):\n{self.trace()[-6000:]}")
            if self.producer:
                self.producer.pump()
            value = predicate()
            if value:
                return value
            time.sleep(0.05)
        raise RuntimeError(f"Timed out: {description}; logs: {self.root}")

    def settle(self, seconds=0.35):
        until = time.monotonic() + seconds
        self.wait(lambda: time.monotonic() >= until, "settled compositor frame", timeout=seconds + 4)

    def region(self, bounds):
        self.wait(lambda: region_matches(self.trace(), bounds), f"committed rounded input bounds {bounds}")
        self.settle()

    def underlying(self, x, y):
        self.clicks += 1
        self.host.click(x, y, 1)
        self.wait(lambda: f"PROBE_UNDERLYING {self.clicks}\n" in self.probe_trace(),
                  f"click-through at {(x, y)}")
        self.settle(0.15)
        self.no_leak()

    def no_leak(self):
        counts = re.findall(r"PROBE_UNDERLYING (\d+)\n", self.probe_trace())
        require((int(counts[-1]) if counts else 0) == self.clicks,
                "Click leaked to underlying surface")

    def header(self, bounds):
        # Scroll to the document start before using its header, including after
        # a collapse preserves/clamps the ScrollView's retained offset.
        self.scroll(4)
        offset = len(self.trace())
        self.host.click(550, 42, 1)
        self.region(bounds)
        if bounds == EXPANDED:
            require("set_keyboard_interactivity(1)" in self.trace()[offset:],
                    "User expansion did not request Exclusive focus")
        self.no_leak()
        surface = native_surface(self.trace())[1]
        require(re.search(rf"wl_pointer[#@]\d+\.enter\([^,]+, wl_surface[#@]{surface},", self.trace()),
                "Pointer never entered the native activity surface")

    def scroll(self, button):
        # Native XTEST wheel events, not an injected widget offset. Twelve
        # notches exceed this short fixture's natural document height.
        for _ in range(12):
            if self.producer:
                self.producer.pump()
            self.host.click(550, 45 if button == 4 else 200, button)
        self.settle()

    def transparent(self, name, points, baseline):
        self.host.move(900, 600)
        self.settle()
        actual = self.host.pixels(points)
        require(actual == baseline, f"{name} opaque margin/corner: {actual} != {baseline}")
        self.pixels[name] = {"points": points, "before": baseline, "after": actual}
        require(self.host.pixels([(550, 12)])[0] != self.background,
                f"{name} capsule did not paint over baseline")

    def launch(self, cycle):
        self.log = self.root / f"host-{cycle}.log"
        self.client = self.spawn([str(self.args.example.resolve()), str(self.directory), "--overlay"],
                                 self.log.name)
        self.wait(lambda: self.endpoint.is_socket() and native_surface(self.trace()), "rendered native activity host")
        self.region(COMPACT)
        output = self.trace()
        require("get_toplevel(" not in output, "Overlay fell back to an ordinary toplevel")
        role, surface, lifetime = native_surface(output)
        for operation in ("set_size(420, 300)", "set_anchor(1)",
                          "set_exclusive_zone(0)", "set_keyboard_interactivity(2)"):
            require(re.search(rf"zwlr_layer_surface_v1[#@]{role}\." + re.escape(operation), lifetime),
                    f"Missing native policy: {operation}")
        require(not re.search(r"set_keyboard_interactivity\([01]\)", output), "Startup must be OnDemand")
        require(re.search(rf"zwlr_layer_surface_v1[#@]{role}\.configure\(\d+, 420, 300\)", lifetime),
                "Unexpected native surface metrics (requires 420x300 at 1x)")
        ack = re.search(rf"zwlr_layer_surface_v1[#@]{role}\.ack_configure\(", lifetime)
        attach = re.search(rf"wl_surface[#@]{surface}\.attach\(", lifetime)
        commit = re.search(rf"wl_surface[#@]{surface}\.commit\(", lifetime)
        require(ack and attach and commit and commit.start() < ack.start() < attach.start(),
                "Native initial commit/configure acknowledgement/buffer order is wrong")
        require(not re.search(r"set_exclusive_zone\((?!0\))-?\d+\)", output), "Unexpected exclusive reservation")
        self.passed(f"host {cycle}: native overlay role, 420x300 configure, initial commit/ack/buffer, OnDemand, zero exclusive zone")

    def connect(self):
        self.producer = Producer(self.endpoint)
        self.producer.start()
        self.passed("raw Unix JSON Hello/Ready and Start/Started (4-byte BE, bounded 16KiB frames)")

    def action(self, expected):
        # At the natural document bottom: viewport bottom 276, Close 236..276,
        # 12px gap, single unwrapped action row 184..224. First action is 88px
        # wide at world x364; x408 is its center, independent of font glyphs.
        self.producer.pump()
        require(not self.producer.actions, f"Unsolicited actions before {expected}: {self.producer.actions}")
        self.host.click(408, 204, 1)
        self.wait(lambda: self.producer.actions, f"real {expected} button -> producer Poll")
        require(self.producer.actions == [{"id": self.producer.activity, "action": expected}],
                f"Wrong real action roundtrip: {self.producer.actions}")
        self.producer.actions.clear()
        self.no_leak()

    def focus(self):
        offset = len(self.probe_trace())
        host_offset = len(self.trace())
        self.host.key("Escape")
        self.wait(lambda: region_matches(self.trace(), COMPACT)
                  or "PROBE_KEY 0" in self.probe_trace()[offset:]
                  or re.search(r"wl_keyboard[#@]\d+\.key\(", self.trace()[host_offset:]),
                  "Escape delivered to host or underlying probe")
        self.settle()
        if region_matches(self.trace(), COMPACT):
            require("PROBE_KEY 0" not in self.probe_trace()[offset:], "Escape delivered to both surfaces")
            require("set_keyboard_interactivity(2)" in self.trace()[host_offset:],
                    "Escape did not restore OnDemand")
            self.passed("bounded expansion focus delivers real Escape; collapse restores OnDemand")
            self.header(EXPANDED)
            return
        raise RuntimeError("Escape did not collapse the host: keyboard delivery/focus acceptance failed")

    def close_host(self, cycle):
        self.scroll(5)
        self.host.click(550, 256, 1)  # Real final 'Close host' control.
        # Stop heartbeats deliberately: shutdown must disconnect this live owner.
        peer, self.producer = self.producer, None
        try:
            self.wait(lambda: self.client.poll() is not None, "normal Close host exit", timeout=12, alive=False)
            require(self.client.returncode == 0, f"Close host failed:\n{self.trace()[-6000:]}")
            require(not self.endpoint.exists(), "Normal host shutdown left activity.sock")
            terminal = peer.shutdown_eof(self.root / f"shutdown-{cycle}-wire.bin")
            (self.root / f"shutdown-{cycle}.json").write_text(json.dumps({
                "exit_code": self.client.returncode, "socket_removed": not self.endpoint.exists(),
                "terminal_reply": terminal, "eof": True,
            }, indent=2) + "\n")
        finally:
            peer.close()
        self.no_leak()
        output = self.trace()
        require(not any(marker in output.lower() for marker in (
            "error in client communication", "activity shutdown cleanup failed", "activity endpoint cleanup failed", "panicked at")),
            f"Host reported a protocol/cleanup failure; see {self.log}")
        require(re.search(r"zwlr_layer_surface_v1[#@]\d+\.destroy\(\)", output), "Native role not destroyed")
        self.passed(f"host {cycle}: real scrolled Close host, exit 0, socket unlinked, live producer EOF, native role destroyed")
        self.client = None

    def reject_x11(self):
        directory = self.root / "x11-rejected"
        directory.mkdir(mode=0o700)
        env = {key: value for key, value in self.env.items()
               if key not in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "WAYLAND_DEBUG")}
        client = self.spawn([str(self.args.example.resolve()), str(directory), "--overlay"],
                            "x11-rejected.log", env)
        require(client.wait(timeout=30) != 0, "X11 overlay silently fell back to a window")
        output = (self.root / "x11-rejected.log").read_text(errors="replace")
        require("wayland" in output.lower() or "layer-shell" in output.lower(),
                f"X11 failed for an unrelated reason: {output}")
        require(not (directory / "activity.sock").exists(), "X11 rejection left the IPC endpoint")
        self.passed("X11 overlay fails explicitly without fallback and removes the bound IPC endpoint")

    def run(self):
        self.reject_x11()
        self.directory = self.root / "activity"
        self.directory.mkdir(mode=0o700)
        self.endpoint = self.directory / "activity.sock"
        self.probe = self.spawn([str(self.args.probe.resolve()), "passive"], "passive.log")
        self.wait(lambda: "PROBE_REGION expanded=false" in self.probe_trace()
                  and len(re.findall(r"wl_surface[#@]\d+\.attach\(", self.probe_trace())) >= 2,
                  "underlying passive probe buffers")
        self.settle(0.7)
        self.host.move(900, 600)
        compact_points = [(344, 40), (431, 9), (668, 9), (550, 90), (350, 290), (550, 298)]
        expanded_points = [(344, 40), (349, 9), (750, 9), (349, 290), (750, 290), (550, 298)]
        compact_baseline = self.host.pixels(compact_points)
        expanded_baseline = self.host.pixels(expanded_points)
        self.background = self.host.pixels([(550, 12)])[0]
        self.underlying(550, 220)
        self.passed("passive fixture accepts underlying click; compositor pixel baseline captured before activity host")
        self.launch(1)
        self.transparent("compact", compact_points, compact_baseline)
        self.underlying(344, 40)
        self.underlying(431, 9)
        self.underlying(370, 140)
        self.host.move(550, 42)
        self.settle(0.5)
        require(region_matches(self.trace(), COMPACT), "Hover expanded the host")
        # Match widget interaction history, not only native keyboard focus.
        # Clicking a header retains its per-widget focus outline even after a
        # wl_keyboard.leave. The post-action collapse has clicked this header;
        # an untouched startup header is therefore not an equivalent baseline.
        self.header(EXPANDED)
        self.header(COMPACT)
        self.underlying(900, 600)
        self.host.move(900, 600)
        empty = self.host.sample(446, 24, 208, 44)
        screenshot(self.host, self.root / "empty-before.ppm")
        self.connect()
        self.wait(lambda: self.host.sample(446, 24, 208, 44) != empty, "producer summary rendered")
        require(region_matches(self.trace(), COMPACT), "IPC expanded the host")
        self.header(EXPANDED)
        self.transparent("expanded", expanded_points, expanded_baseline)
        self.underlying(344, 40)
        self.underlying(349, 9)
        # Re-click the header after click-through displaced keyboard focus.
        self.header(COMPACT)
        self.underlying(370, 140)
        self.header(EXPANDED)
        self.focus()
        self.passed("compact/expanded compositor pixels preserve baseline at margins/corners; rounded pointer pass-through, click consumption and region updates; no hover/IPC expansion")
        self.scroll(5)
        self.action("Pause")
        self.producer.replace("Paused")
        self.settle()
        self.scroll(5)
        self.action("Resume")
        self.producer.replace("Running")
        self.settle()
        self.passed("native wheel reaches natural details; real Pause and Resume -> Poll action IDs -> full Replace/Accepted")
        self.header(COMPACT)
        # Match the baseline's clicked, unhovered header and underlying native
        # keyboard focus. Widget focus is intentionally retained in both captures.
        self.underlying(900, 600)
        self.host.move(900, 600)
        screenshot(self.host, self.root / "populated-before-disconnect.ppm")
        self.producer.close()
        self.producer = None
        try:
            self.wait(lambda: self.host.sample(446, 24, 208, 44) == empty,
                      "disconnect removes selected activity and restores empty host summary")
        finally:
            screenshot(self.host, self.root / "empty-after.ppm")
            actual = self.host.sample(446, 24, 208, 44)
            (self.root / "disconnect-samples.json").write_text(json.dumps({
                "rect": [446, 24, 208, 44], "sample_stride": 2,
                "before": empty, "after": actual,
                "different": sum(a != b for a, b in zip(empty, actual)),
            }, indent=2) + "\n")
        self.passed("producer disconnect removes visible selected activity (empty-summary pixel equality)")
        self.connect()
        self.header(EXPANDED)
        self.close_host(1)
        self.launch(2)
        self.connect()
        self.header(EXPANDED)
        self.close_host(2)
        self.passed("same private endpoint rebinds after normal host exit; fresh producer starts and second real Close succeeds")


def owned_environment(root):
    # Allowlist instead of removing selected session variables: no inherited bus,
    # desktop socket, config, autostart, preload, toolkit or renderer selection.
    env = {key: os.environ[key] for key in ("PATH", "LD_LIBRARY_PATH", "LANG", "LC_ALL", "TZ")
           if key in os.environ}
    for directory in ("home", "config", "cache", "data"):
        (root / directory).mkdir(mode=0o700)
    env.update(HOME=str(root / "home"), XDG_RUNTIME_DIR=str(root),
               XDG_CONFIG_HOME=str(root / "config"), XDG_CONFIG_DIRS=str(root / "config"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"),
               LIBGL_ALWAYS_SOFTWARE="1", WINIT_X11_SCALE_FACTOR="1")
    return env


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    compositor = parser.add_mutually_exclusive_group(required=True)
    compositor.add_argument("--labwc", type=Path)
    compositor.add_argument("--fraym", type=Path, help="Existing binary, owned private IPC")
    parser.add_argument("--xvfb", type=Path, required=True)
    parser.add_argument("--example", type=Path, default=Path("target/debug/examples/dynamic_island_host"))
    parser.add_argument("--probe", type=Path, default=Path("target/debug/examples/dynamic_island_probe"))
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    for binary in (args.labwc or args.fraym, args.xvfb, args.example, args.probe):
        require(binary.is_file() and os.access(binary, os.X_OK), f"Missing executable: {binary}")
    root = Path(tempfile.mkdtemp(prefix="rutter-activity-", dir="/tmp/opencode"))
    children, host, smoke, lock, success = [], None, None, None, False

    def expired(_signal, _frame):
        raise TimeoutError("Global 160-second smoke deadline exceeded (cleanup follows)")

    previous = signal.signal(signal.SIGALRM, expired)
    signal.alarm(160)  # Leave approximately 20s for sequential exact-process cleanup.
    try:
        env = owned_environment(root)
        (root / "autostart").write_text("# Owned compositor: no autostart.\n")
        (root / "environment").write_text("# No inherited desktop/session.\n")
        (root / "rc.xml").write_text('<labwc_config><mouse><default/></mouse><keyboard><default/></keyboard><windowRules><windowRule title="Rutter input probe underlying"><action name="Maximize"/></windowRule></windowRules></labwc_config>')
        lock = (root / "wayland-0.lock").open("w")
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        number = next(n for n in range(98, 120) if not Path(f"/tmp/.X11-unix/X{n}").exists()
                      and not Path(f"/tmp/.X{n}-lock").exists())
        display = f":{number}"
        with (root / "xvfb.log").open("w") as log:
            xserver = subprocess.Popen([str(args.xvfb.resolve()), display, "-screen", "0", "1100x800x24",
                                        "-nolisten", "tcp", "-ac", "-noreset"], env=env,
                                       stdout=log, stderr=subprocess.STDOUT)
        children.append(xserver)
        # Reuse the same bounded waiter before a GUI/producer exists.
        smoke = Smoke(root, env, args, children, None)
        smoke.wait(lambda: xserver.poll() is not None or Path(f"/tmp/.X11-unix/X{number}").exists(), "owned Xvfb")
        require(xserver.poll() is None, "Owned Xvfb failed to start; see xvfb.log")
        env.update(DISPLAY=display, WLR_BACKENDS="x11", WLR_RENDERER="pixman", WLR_X11_OUTPUTS="1")
        (root / "config.toml").write_text("[dpms]\nenabled = false\n[layout]\nouter_gap = 0\ninner_gap = 0\nborder_width = 0\n")
        env.update(FRAYM_CONFIG=str(root / "config.toml"), FRAYM_SOCKET=str(root / "ipc.sock"))
        command = ([str(args.labwc.resolve()), "-C", str(root), "-d"] if args.labwc
                   else [str(args.fraym.resolve()), "--nested"])
        owner = smoke.spawn(command, "compositor.log")
        if args.fraym:
            smoke.wait(lambda: (root / "ipc.sock").is_socket(), "owned FRAYM IPC")
            ctl(root / "ipc.sock")
        match = smoke.wait(lambda: re.search(r"(?:WAYLAND_DISPLAY=|Socket Wayland: )(\S+)",
                                             (root / "compositor.log").read_text()), "owned compositor socket")
        wayland = match[1]
        require(not Path(wayland).is_absolute() and "/" not in wayland and wayland != "wayland-0"
                and (root / wayland).is_socket(), "Compositor did not advertise an owned private socket")
        env.update(WAYLAND_DISPLAY=wayland, WAYLAND_DEBUG="client",
                   VK_DRIVER_FILES=str(root / "disabled-vulkan.json"),
                   VK_ICD_FILENAMES=str(root / "disabled-vulkan.json"))
        host = PopupInput(display)
        smoke.host = host
        host.window = smoke.wait(lambda: host_window(host.display, display, owner.pid, bool(args.labwc)),
                                 "owned compositor X11 window")
        host.x.XResizeWindow(host.display, host.window, 1100, 800)
        host.x.XSetInputFocus(host.display, host.window, 2, 0)
        host.x.XFlush(host.display)
        smoke.settle(0.5)
        smoke.run()
        success = True
    finally:
        signal.alarm(0)
        if smoke and smoke.producer:
            smoke.producer.close()
        if host:
            host.close()
        for child in reversed(children):
            stop(child)
        if lock:
            lock.close()
        signal.signal(signal.SIGALRM, previous)
        report = {"result": ("qualified" if smoke and smoke.limitations else "pass") if success else "fail",
                  "checks": smoke.checks if smoke else [],
                  "limitations": (smoke.limitations if smoke else []) + [
                      "Automated compositor sampling only; no manual visual acceptance.",
                      "Only 1100x800 output, 420x300 host, 1x, default light theme and short ASCII transfer.",
                      "Owner registry internals are not exposed: disconnect cleanup checked by empty-summary pixels; shutdown checked by EOF, exit, socket removal and absence of cleanup errors.",
                      "X11 rejection is covered; a compositor missing layer-shell or v4 was not run."],
                  "pixels": smoke.pixels if smoke else {}}
        (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"RESULT: {report['result']}; {len(report['checks'])} completed checks", flush=True)
        if success and not args.keep and not smoke.limitations:
            shutil.rmtree(root)
        else:
            print(f"Logs and actual compositor pixel comparisons: {root}", flush=True)


if __name__ == "__main__":
    main()
