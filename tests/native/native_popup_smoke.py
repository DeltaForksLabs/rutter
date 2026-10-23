"""Exercise the clock/calendar example on owned Xvfb + nested labwc displays.

Build first (sequentially): cargo build --locked --offline --example native_popup_clock
Requires labwc, Xvfb, wlr-randr, XTEST, software EGL and the example binary.
No inherited desktop or session bus is used. Logs are retained on failure.
"""

import argparse
import ctypes as C
import fcntl
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time

from layer_shell_smoke import HostInput, host_window, stop, wait_for


WIDTH, HEIGHT = 1600, 1200


class PopupInput(HostInput):
    def move(self, x, y):
        self.x.XWarpPointer(self.display, 0, self.window, 0, 0, 0, 0, x, y)
        self.x.XFlush(self.display)
        time.sleep(0.15)

    def button(self, pressed):
        assert self.xt.XTestFakeButtonEvent(self.display, 1, int(pressed), 0)
        self.x.XFlush(self.display)

    def key(self, symbol):
        self.x.XStringToKeysym.argtypes = [C.c_char_p]
        self.x.XStringToKeysym.restype = C.c_ulong
        self.x.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
        self.x.XKeysymToKeycode.restype = C.c_uint
        self.xt.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
        code = self.x.XKeysymToKeycode(self.display, self.x.XStringToKeysym(symbol.encode()))
        assert code
        for pressed in (1, 0):
            assert self.xt.XTestFakeKeyEvent(self.display, code, pressed, 0)
        self.x.XFlush(self.display)

    def sample(self, x, y, width, height):
        assert 0 <= x < x + width <= WIDTH and 0 <= y < y + height <= HEIGHT
        self.x.XGetImage.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_ulong, C.c_int]
        self.x.XGetImage.restype = C.c_void_p
        self.x.XGetPixel.argtypes = [C.c_void_p, C.c_int, C.c_int]
        self.x.XGetPixel.restype = C.c_ulong
        self.x.XDestroyImage.argtypes = [C.c_void_p]
        image = self.x.XGetImage(self.display, self.window, x, y, width, height, C.c_ulong(-1).value, 2)
        assert image
        try:
            return [self.x.XGetPixel(image, dx, dy) for dy in range(0, height, 2) for dx in range(0, width, 2)]
        finally:
            self.x.XDestroyImage(image)


def check_example(host, env, binary, edge, scale, log_path):
    parent = {
        "top": (0, 0), "bottom": (0, HEIGHT - round(32 * scale)),
        "left": (0, 0), "right": (WIDTH - round(32 * scale), 0),
    }[edge]
    anchor = (parent[0] + round(16 * scale), parent[1] + round(16 * scale))
    with log_path.open("w") as log:
        client = subprocess.Popen([str(binary), edge], env=env, stdout=log, stderr=subprocess.STDOUT)
    trace = lambda: log_path.read_text()
    try:
        wait_for(lambda: re.search(r"wl_surface[#@]\d+\.attach\(", trace()), "panel's first rendered buffer")
        time.sleep(0.3)
        # Backend selection may probe and retire an initial parent role. Opening
        # the actual child must not allocate any further layer roles.
        parent_roles = len(re.findall(r"get_layer_surface\(", trace()))
        for cycle in range(3):
            before = len(re.findall(r"get_popup\(new id xdg_popup", trace()))
            host.move(*anchor)
            host.button(True)
            try:
                # Keep the physical press alive through synchronous construction.
                wait_for(lambda: len(re.findall(r"get_popup\(new id xdg_popup", trace())) > before, "native popup role")
                wait_for(lambda: len(re.findall(r"xdg_popup[#@]\d+\.grab\(", trace())) > before, "required native grab")
            finally:
                host.button(False)
            time.sleep(0.5)
            configures = re.findall(r"xdg_popup[#@]\d+\.configure\((-?\d+), (-?\d+), (\d+), (\d+)\)", trace())
            assert configures, trace()
            x, y, width, height = map(int, configures[-1])
            assert (width, height) == (384, 448), configures[-1]
            origin = (parent[0] + round(x * scale), parent[1] + round(y * scale))
            assert 0 <= origin[0] <= WIDTH - round(width * scale)
            assert 0 <= origin[1] <= HEIGHT - round(height * scale)

            def sample(rect):
                dx, dy, w, h = rect
                return host.sample(origin[0] + round(dx * scale), origin[1] + round(dy * scale), round(w * scale), round(h * scale))

            def click(dx, dy):
                host.move(origin[0] + round(dx * scale), origin[1] + round(dy * scale))
                host.button(True)
                time.sleep(0.15)
                host.button(False)
                host.move(*anchor)
                time.sleep(0.2)

            if cycle == 0:
                clock_before = sample((12, 12, 360, 38))
                wait_for(lambda: sample((12, 12, 360, 38)) != clock_before, "visible HH:MM:SS ticking", timeout=4)
                heading_before = sample((100, 66, 180, 34))
                click(309, 83)  # Calendar's next-month button, not the clock.
                assert sample((100, 66, 180, 34)) != heading_before, "month heading did not change"
                days_before = sample((20, 132, 344, 236))
                click(44, 190)  # An in-month day in the second calendar week.
                assert sample((20, 132, 344, 236)) != days_before, "selected day did not repaint"
                host.key("Escape")
            elif cycle == 1:
                outside = next((px, py) for px, py in [(800, 600), (10, 1190), (1590, 1190)]
                               if not (origin[0] <= px < origin[0] + width * scale and origin[1] <= py < origin[1] + height * scale))
                host.move(*outside)
                host.button(True)
                time.sleep(0.15)
                host.button(False)
            else:
                click(180, 410)  # The example's explicit Close button.
            wait_for(lambda: len(re.findall(r"xdg_popup[#@]\d+\.destroy\(\)", trace())) > cycle, "popup dismissal and route retirement")
            assert client.poll() is None, trace()
        output = trace()
        assert len(re.findall(r"get_layer_surface\(", output)) == parent_roles, "child must not become another layer surface"
        assert len(re.findall(r"get_popup\(new id xdg_popup[#@]\d+, nil,", output)) == 3
        assert len(re.findall(r"zwlr_layer_surface_v1[#@]\d+\.get_popup\(", output)) == 3
        assert "error in client communication" not in output.lower()
        print(f"PASS: {edge} at {scale}x: native role/grab, configured bounds, ticking, month/day pixels, Escape/outside/Close and reopen", flush=True)
    finally:
        stop(client)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--labwc", type=Path, required=True)
    parser.add_argument("--xvfb", type=Path, required=True)
    parser.add_argument("--wlr-randr", type=Path, required=True)
    parser.add_argument("--example", type=Path, default=Path("target/debug/examples/native_popup_clock"))
    parser.add_argument("--edges", nargs="+", choices=["top", "bottom", "left", "right"], default=["top", "bottom", "left", "right"])
    parser.add_argument("--scales", nargs="+", type=float, choices=[1.0, 1.25, 2.0], default=[1.0, 1.25, 2.0])
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix="rutter-popup-", dir="/tmp/opencode"))
    children, host, success = [], None, False
    env = {k: v for k, v in os.environ.items() if not k.startswith(("DBUS_", "FRAYM_", "WLR_")) and k not in {
        "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "WAYLAND_DEBUG", "XAUTHORITY",
        "SESSION_MANAGER", "XDG_SESSION_ID", "XDG_SESSION_TYPE", "XDG_SESSION_DESKTOP",
        "XDG_CURRENT_DESKTOP", "DESKTOP_SESSION", "DESKTOP_AUTOSTART_ID",
    }}
    env.update(XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root), XDG_CONFIG_DIRS=str(root), LIBGL_ALWAYS_SOFTWARE="1")
    (root / "autostart").write_text("# No session autostart in the owned compositor.\n")
    (root / "environment").write_text("# No inherited session environment.\n")
    lock = (root / "wayland-0.lock").open("w")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    try:
        number = next(n for n in range(98, 120) if not Path(f"/tmp/.X11-unix/X{n}").exists() and not Path(f"/tmp/.X{n}-lock").exists())
        display = f":{number}"
        with (root / "xvfb.log").open("w") as log:
            children.append(subprocess.Popen([str(args.xvfb.resolve()), display, "-screen", "0", f"{WIDTH}x{HEIGHT}x24", "-nolisten", "tcp", "-ac", "-noreset"], env=env, stdout=log, stderr=subprocess.STDOUT))
        wait_for(lambda: Path(f"/tmp/.X11-unix/X{number}").exists(), "owned Xvfb display")
        env.update(DISPLAY=display, WLR_BACKENDS="x11", WLR_RENDERER="pixman", WLR_X11_OUTPUTS="1")
        with (root / "labwc.log").open("w") as log:
            compositor = subprocess.Popen([str(args.labwc.resolve()), "-C", str(root), "-d"], env=env, stdout=log, stderr=subprocess.STDOUT)
        children.append(compositor)
        wayland = wait_for(lambda: re.search(r"WAYLAND_DISPLAY=(\S+)", (root / "labwc.log").read_text()), "owned labwc socket")[1]
        assert wayland != "wayland-0"
        env["WAYLAND_DISPLAY"] = wayland
        host = PopupInput(display)
        host.window = wait_for(lambda: host_window(host.display, display, compositor.pid, True), "owned labwc host window")
        host.x.XResizeWindow(host.display, host.window, WIDTH, HEIGHT)
        host.x.XSetInputFocus(host.display, host.window, 2, 0)
        host.x.XFlush(host.display)
        time.sleep(0.5)
        client_env = env.copy()
        # Arboard's clipboard still needs the owned X11 server; Winit selects
        # Wayland when its socket is present (asserted by the protocol trace).
        client_env["WAYLAND_DEBUG"] = "client"
        for scale in args.scales:
            subprocess.run([str(args.wlr_randr.resolve()), "--output", "X11-1", "--scale", str(scale)], env=env, check=True, timeout=5)
            time.sleep(0.3)
            for edge in args.edges:
                check_example(host, client_env, args.example.resolve(), edge, scale, root / f"{edge}-{scale}.log")
        success = True
    finally:
        if host:
            host.close()
        for child in reversed(children):
            stop(child)
        lock.close()
        if success and not args.keep:
            shutil.rmtree(root)
        else:
            print(f"Logs: {root}", flush=True)


if __name__ == "__main__":
    main()
