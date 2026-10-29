"""Request64.3: genuine input and GL composition on an owned nested compositor.

Build first: CARGO_BUILD_JOBS=2 cargo build --locked --offline --example dynamic_island_probe
Run with --labwc /absolute/path/to/labwc --xvfb /absolute/path/to/Xvfb.
Private shared libraries can be supplied through LD_LIBRARY_PATH. No pip dependencies.
Only 1100x800 at 1x is covered; this is not manual visual acceptance.
"""

import argparse
import ctypes as C
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time

# Also works when loaded by an external runner; never leave repository pycache.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layer_shell_smoke import ctl, host_window, stop, wait_for
from native_popup_smoke import PopupInput


def screenshot(host, path):
    """Save the actual compositor's capsule-area pixels, not a software widget raster."""
    host.pixels([(550, 20)])  # Configure XImage signatures and synchronize.
    image = host.x.XGetImage(host.display, host.window, 340, 0, 420, 300, C.c_ulong(-1).value, 2)
    assert image, "Unable to capture owned compositor"
    try:
        rgb = bytearray()
        for y in range(300):
            for x in range(420):
                pixel = host.x.XGetPixel(image, x, y)
                rgb.extend(((pixel >> 16) & 255, (pixel >> 8) & 255, pixel & 255))
        path.write_bytes(b"P6\n420 300\n255\n" + rgb)
    finally:
        host.x.XDestroyImage(image)


def check_client(host, env, binary, mode, root):
    log_path = root / f"{mode}.log"
    with log_path.open("w") as log:
        client = subprocess.Popen([str(binary), mode], env=env, stdout=log, stderr=subprocess.STDOUT)

    def trace():
        output = log_path.read_text()
        assert client.poll() is None, f"Fixture exited ({client.returncode}):\n{output[-6000:]}"
        return output

    def expect(marker, offset=0):
        return wait_for(lambda: marker in trace()[offset:], marker, timeout=12)

    def click(x, y, marker):
        offset = len(trace())
        host.click(x, y, 1)
        expect(marker, offset)
        time.sleep(0.2)  # Settle the native region commit and GL frame.

    try:
        keyboard_failure = None
        expect("PROBE_REGION expanded=false")
        wait_for(lambda: len(re.findall(r"wl_surface[#@]\d+\.attach\(", trace())) >= 2, "both native buffers")
        time.sleep(0.7)
        output = trace()
        assert "get_toplevel(" in output and "get_layer_surface(" in output
        keyboard = 0 if mode == "passive" else 2
        assert re.search(rf"set_keyboard_interactivity\({keyboard}\)", output), "Wrong layer keyboard policy"
        # Origin is (340,0), bounds (8,8,404,76). Avoid fonts and rounded antialiasing.
        points = [(344, 40), (349, 9), (352, 45), (550, 220)]
        colors = host.pixels(points)
        print(mode, "compositor pixels", [hex(c) for c in colors], flush=True)
        assert colors[0] == colors[1] == colors[3] == 0x20A060, colors
        # Expected half-alpha blend: (32,112,160), tolerance for GL rounding.
        channels = [(colors[2] >> shift) & 255 for shift in (16, 8, 0)]
        assert all(abs(a - b) <= 2 for a, b in zip(channels, (32, 112, 160))), channels
        screenshot(host, root / f"{mode}-collapsed.ppm")
        # Probe initial policy separately from reacquiring focus after a click
        # through the region. Some compositor versions treat these differently.
        offset = len(trace())
        host.key("Escape")
        expect(f"PROBE_KEY {0 if mode == 'passive' else 1}", offset)
        count = 0

        def underlying(x, y):
            nonlocal count
            count += 1
            click(x, y, f"PROBE_UNDERLYING {count}\n")

        underlying(344, 40)  # Transparent host padding.
        underlying(349, 9)   # Inside capsule bounding box, outside rounded corner.
        if mode == "passive":
            underlying(550, 45)  # Through visibly painted capsule, not only alpha=0.
            offset = len(trace())
            host.key("Escape")
            expect("PROBE_KEY 0", offset)
            assert "PROBE_KEY 1" not in trace()
            assert "PROBE_ACTION" not in trace() and "expanded=true" not in trace()
            print("PASS: passive Empty: padding/corner/painted-capsule click-through; None preserves underlying keyboard", flush=True)
        else:
            click(550, 45, "PROBE_REGION expanded=true")
            offset = len(trace())
            host.key("Escape")
            wait_for(lambda: "PROBE_KEY " in trace()[offset:], "keyboard delivery")
            if "PROBE_KEY 1" not in trace()[offset:]:
                keyboard_failure = "OnDemand: capsule click did not reacquire keyboard focus (Escape reached underlying surface)"
            screenshot(host, root / "interactive-expanded.ppm")
            click(550, 102, "PROBE_ACTION")
            assert f"PROBE_UNDERLYING {count + 1}\n" not in trace(), "Capsule button leaked to underlying window"
            click(550, 45, "PROBE_REGION expanded=false")
            underlying(550, 102)  # Former expanded action is now outside native region.
            offset = len(trace())
            host.key("Escape")
            expect("PROBE_KEY 0", offset)
            click(550, 45, "PROBE_REGION expanded=true")
            click(550, 102, "PROBE_ACTION")
            assert f"PROBE_UNDERLYING {count + 1}\n" not in trace()
            print("PASS: rounded interactive region excludes padding/corner; header/action accepted; expand/collapse/reopen updates native region", flush=True)
        output = trace()
        assert "wl_region" in output and ".set_input_region(" in output
        assert "error in client communication" not in output.lower()
        (root / f"{mode}-pixels.json").write_text(json.dumps(dict(points=points, rgb=colors)))
        assert keyboard_failure is None, keyboard_failure
    finally:
        stop(client)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    compositor = parser.add_mutually_exclusive_group(required=True)
    compositor.add_argument("--labwc", type=Path)
    compositor.add_argument("--fraym", type=Path, help="Existing binary only; uses private IPC")
    parser.add_argument("--xvfb", type=Path, required=True)
    parser.add_argument("--example", type=Path, default=Path("target/debug/examples/dynamic_island_probe"))
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    for binary in (args.labwc or args.fraym, args.xvfb, args.example):
        assert binary.is_file() and os.access(binary, os.X_OK), f"Missing executable: {binary}"
    root = Path(tempfile.mkdtemp(prefix="rutter-island-", dir="/tmp/opencode"))
    children, host, success = [], None, False
    env = {k: v for k, v in os.environ.items() if not k.startswith(("DBUS_", "FRAYM_", "WLR_", "RUTTER_")) and k not in {
        "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "WAYLAND_DEBUG", "XAUTHORITY",
        "SESSION_MANAGER", "XDG_SESSION_ID", "XDG_SESSION_TYPE", "XDG_SESSION_DESKTOP",
        "XDG_CURRENT_DESKTOP", "DESKTOP_SESSION", "DESKTOP_AUTOSTART_ID",
    }}
    env.update(XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root), XDG_CONFIG_DIRS=str(root), LIBGL_ALWAYS_SOFTWARE="1", CARGO_BUILD_JOBS="2")
    (root / "autostart").write_text("# Owned compositor, no autostart.\n")
    (root / "environment").write_text("# No inherited session environment.\n")
    (root / "rc.xml").write_text('<labwc_config><mouse><default/></mouse><keyboard><default/></keyboard><windowRules><windowRule title="Rutter input probe underlying"><action name="Maximize"/></windowRule></windowRules></labwc_config>')
    lock = (root / "wayland-0.lock").open("w")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    try:
        number = next(n for n in range(98, 120) if not Path(f"/tmp/.X11-unix/X{n}").exists() and not Path(f"/tmp/.X{n}-lock").exists())
        display = f":{number}"
        with (root / "xvfb.log").open("w") as log:
            children.append(subprocess.Popen([str(args.xvfb.resolve()), display, "-screen", "0", "1100x800x24", "-nolisten", "tcp", "-ac", "-noreset"], env=env, stdout=log, stderr=subprocess.STDOUT))
        wait_for(lambda: Path(f"/tmp/.X11-unix/X{number}").exists(), "owned Xvfb")
        env.update(DISPLAY=display, WLR_BACKENDS="x11", WLR_RENDERER="pixman", WLR_X11_OUTPUTS="1", WINIT_X11_SCALE_FACTOR="1")
        (root / "config.toml").write_text("[dpms]\nenabled = false\n[layout]\nouter_gap = 0\ninner_gap = 0\nborder_width = 0\n")
        env.update(FRAYM_CONFIG=str(root / "config.toml"), FRAYM_SOCKET=str(root / "ipc.sock"))
        command = [str(args.labwc.resolve()), "-C", str(root), "-d"] if args.labwc else [str(args.fraym.resolve()), "--nested"]
        with (root / "compositor.log").open("w") as log:
            compositor = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
        children.append(compositor)
        if args.fraym:
            wait_for(lambda: (root / "ipc.sock").exists(), "owned FRAYM IPC")
            ctl(root / "ipc.sock")
        wayland = wait_for(lambda: re.search(r"(?:WAYLAND_DISPLAY=|Socket Wayland: )(\S+)", (root / "compositor.log").read_text()), "owned compositor socket")[1]
        assert wayland != "wayland-0" and (root / wayland).is_socket()
        env.update(WAYLAND_DISPLAY=wayland, WAYLAND_DEBUG="client")
        # Disable Vulkan via the loader, not a production test hook. Transparent
        # surfaces reject CPU fallback, so successful presentation requires GL.
        env.update(VK_DRIVER_FILES=str(root / "disabled-vulkan.json"), VK_ICD_FILENAMES=str(root / "disabled-vulkan.json"))
        host = PopupInput(display)
        host.window = wait_for(lambda: host_window(host.display, display, compositor.pid, bool(args.labwc)), "owned compositor host")
        host.x.XResizeWindow(host.display, host.window, 1100, 800)
        host.x.XSetInputFocus(host.display, host.window, 2, 0)
        host.x.XFlush(host.display)
        time.sleep(0.5)
        for mode in ("passive", "interactive"):
            check_client(host, env, args.example.resolve(), mode, root)
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
            print(f"Logs and compositor screenshots: {root}", flush=True)


if __name__ == "__main__":
    main()
