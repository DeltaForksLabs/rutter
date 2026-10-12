"""Run sequential layer-shell checks on owned Weston/Xwayland/FRAYM displays.

Build library tests first: CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib --no-run
Then: python3 tests/native/layer_shell_smoke.py --fraym /absolute/path/to/fraym
Never connects to the inherited desktop. Requires Weston, Xwayland and software EGL.
"""
import argparse
import ctypes as C
import fcntl
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time


def wait_for(predicate, description, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise RuntimeError(f"Timed out: {description}")


def stop(child):
    if child and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()


def ctl(path, request="state"):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(3)
        connection.connect(str(path))
        connection.sendall((json.dumps({"version": 1, "request": request, "args": {}}) + "\n").encode())
        with connection.makefile("r") as reader:
            reply = json.loads(reader.readline())
    assert reply.get("ok"), reply
    return reply


def host_window(display, display_name, pid, labwc=False):
    x = C.CDLL("libX11.so.6")
    x.XDefaultRootWindow.argtypes = [C.c_void_p]
    x.XDefaultRootWindow.restype = C.c_ulong
    x.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong), C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)]
    x.XQueryTree.restype = C.c_int
    x.XFree.argtypes = [C.c_void_p]
    root, parent, children, count = C.c_ulong(), C.c_ulong(), C.POINTER(C.c_ulong)(), C.c_uint()
    assert x.XQueryTree(display, x.XDefaultRootWindow(display), C.byref(root), C.byref(parent), C.byref(children), C.byref(count))
    try:
        # wlroots' X11 backend does not publish WM_NAME/PID on this version.
        # This private X server has exactly one host client, started by this driver.
        if labwc and count.value in (1, 2):
            return children[0]
        for index in range(count.value):
            window = children[index]
            prop = subprocess.run(["xprop", "-display", display_name, "-id", str(window), "_NET_WM_PID", "WM_NAME"], capture_output=True, text=True, timeout=2)
            if re.search(r"=\s*" + str(pid) + r"\s*$", prop.stdout, re.M):
                return window
            if labwc and any(name in prop.stdout.lower() for name in ('wlroots', 'labwc')):
                return window
    finally:
        x.XFree(children)
    return None


class HostInput:
    def __init__(self, display_name):
        self.x = C.CDLL("libX11.so.6")
        self.xt = C.CDLL("libXtst.so.6")
        signatures = {
            "XOpenDisplay": ([C.c_char_p], C.c_void_p),
            "XCloseDisplay": ([C.c_void_p], C.c_int),
            "XRaiseWindow": ([C.c_void_p, C.c_ulong], C.c_int),
            "XSetInputFocus": ([C.c_void_p, C.c_ulong, C.c_int, C.c_ulong], C.c_int),
            "XWarpPointer": ([C.c_void_p, C.c_ulong, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_int, C.c_int], C.c_int),
            "XResizeWindow": ([C.c_void_p, C.c_ulong, C.c_uint, C.c_uint], C.c_int),
            "XDestroyWindow": ([C.c_void_p, C.c_ulong], C.c_int),
            "XFlush": ([C.c_void_p], C.c_int),
        }
        for name, (args, result) in signatures.items():
            getattr(self.x, name).argtypes = args
            getattr(self.x, name).restype = result
        self.xt.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
        self.xt.XTestFakeButtonEvent.restype = C.c_int
        self.display = self.x.XOpenDisplay(display_name.encode())
        assert self.display
        self.window = None

    def click(self, x, y, button):
        self.x.XRaiseWindow(self.display, self.window)
        self.x.XSetInputFocus(self.display, self.window, 2, 0)
        self.x.XWarpPointer(self.display, 0, self.window, 0, 0, 0, 0, x, y)
        self.x.XFlush(self.display)
        time.sleep(0.15)
        for pressed in (1, 0):
            assert self.xt.XTestFakeButtonEvent(self.display, button, pressed, 0)
        self.x.XFlush(self.display)

    def close(self):
        self.x.XCloseDisplay(self.display)

    def pixels(self, points):
        self.x.XRaiseWindow(self.display, self.window)
        self.x.XFlush(self.display)
        time.sleep(0.1)
        self.x.XGetImage.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_ulong, C.c_int]
        self.x.XGetImage.restype = C.c_void_p
        self.x.XGetPixel.argtypes = [C.c_void_p, C.c_int, C.c_int]
        self.x.XGetPixel.restype = C.c_ulong
        self.x.XDestroyImage.argtypes = [C.c_void_p]
        image = self.x.XGetImage(self.display, self.window, 0, 0, 1100, 800, C.c_ulong(-1).value, 2)
        assert image
        try:
            return [self.x.XGetPixel(image, x, y) & 0xffffff for x, y in points]
        finally:
            self.x.XDestroyImage(image)


def validate_protocol(trace):
    # A real layer role, with initial empty commit then ack, must precede buffers.
    roles = list(re.finditer(r"get_layer_surface\(new id zwlr_layer_surface_v1[#@](\d+), wl_surface[#@](\d+)", trace))
    assert len(roles) >= 4, f"Expected four native layer panels, got {len(roles)}"
    buffered = 0
    for match in roles:
        role, surface = match.groups()
        lifetime = trace[match.start():]
        destroy = re.search(rf"wl_surface[#@]{surface}\.destroy\(\)", lifetime)
        if destroy:
            lifetime = lifetime[:destroy.end()]
        ack = re.search(rf"zwlr_layer_surface_v1[#@]{role}\.ack_configure\(", lifetime)
        attach = re.search(rf"wl_surface[#@]{surface}\.attach\(", lifetime)
        commit = re.search(rf"wl_surface[#@]{surface}\.commit\(", lifetime)
        assert ack and commit and commit.start() < ack.start(), (role, surface)
        if attach:
            assert ack.start() < attach.start(), (role, surface)
            buffered += 1
        else:
            assert destroy, "Failed graphics probe must release its unused layer role"
    assert buffered == 4, f"Expected four rendered layer panels, got {buffered}"
    assert re.search(r"get_popup\(new id xdg_popup[#@]\d+, nil,", trace), "Layer popup needs null xdg parent"
    association = re.search(r"zwlr_layer_surface_v1[#@]\d+\.get_popup\(xdg_popup[#@](\d+)\)", trace)
    assert association, "Layer parent must associate popup before mapping"
    popup = association[1]
    creation = re.search(rf"get_popup\(new id xdg_popup[#@]{popup}, nil,", trace)
    assert creation and creation.start() < association.start()
    assert re.search(r"zwlr_layer_surface_v1[#@]\d+\.destroy\(\)", trace), "Closing must release the role/reservation"
    print("PASS: real layer roles, empty commit/configure ack/buffer ordering, null-parent popup association and destruction")


def main():
    parser = argparse.ArgumentParser()
    compositor = parser.add_mutually_exclusive_group(required=True)
    compositor.add_argument("--fraym", type=Path)
    compositor.add_argument("--labwc", type=Path)
    parser.add_argument("--remove-output", action="store_true", help="Disable labwc's output and verify wl_output removal retires targeted routes")
    parser.add_argument("--wlr-randr", type=Path, help="Additionally exercise integer and fractional output scale through this tool")
    parser.add_argument("--keep", action="store_true")
    retained = parser.add_mutually_exclusive_group()
    retained.add_argument("--retained-layer", action="store_true", help="Run the fork consumer that ignores CloseRequested and retains windows/states")
    retained.add_argument("--retained-renderer", action="store_true", help="Retain OpenGL renderers across targeted output removal and verify cleanup")
    args = parser.parse_args()
    binary = args.fraym or args.labwc
    assert not args.remove_output or (args.labwc and args.wlr_randr), "Output removal probe requires labwc and --wlr-randr"
    assert not args.retained_renderer or args.remove_output, "Retained renderer probe requires --remove-output"
    assert binary.is_absolute() and os.access(binary, os.X_OK)
    root = Path(tempfile.mkdtemp(prefix="rutter-layer-", dir="/tmp/opencode"))
    # Keep automatic compositor naming away from wayland-0, even in this private directory.
    socket_zero_lock = (root / "wayland-0.lock").open("w")
    fcntl.flock(socket_zero_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    env = {key: value for key, value in os.environ.items() if not key.startswith("FRAYM_") and key not in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "XAUTHORITY", "WAYLAND_DEBUG", "RUTTER_LAYER_OUTPUT", "RUTTER_LAYER_LIVE_INPUT", "RUTTER_LAYER_REMOVE_OUTPUT", "RUTTER_LAYER_MAXIMIZE", "DBUS_SESSION_BUS_ADDRESS", "DBUS_STARTER_ADDRESS")}
    env.update(XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root), XDG_CONFIG_DIRS=str(root), LIBGL_ALWAYS_SOFTWARE="1", CARGO_BUILD_JOBS="2", XWAYLAND_NO_GLAMOR="1")
    (root / "autostart").write_text("# Owned disposable compositor: no session autostart.\n")
    (root / "environment").write_text("# No inherited compositor-specific session configuration.\n")
    children = []
    host = None
    success = False
    try:
        weston = subprocess.Popen(["weston", "--backend=headless", "--renderer=pixman", "--width=1100", "--height=800", "--socket=rutter-layer-host", "--no-config", f"--log={root / 'weston.log'}"], env=env)
        children.append(weston)
        wait_for(lambda: (root / "rutter-layer-host").exists(), "owned Weston socket")
        env["WAYLAND_DISPLAY"] = "rutter-layer-host"
        number = next(n for n in range(98, 120) if not Path(f"/tmp/.X11-unix/X{n}").exists() and not Path(f"/tmp/.X{n}-lock").exists())
        display = f":{number}"
        xserver = subprocess.Popen(["Xwayland", display, "-shm", "-ac", "-noreset", "-geometry", "1100x800"], env=env, stdout=subprocess.DEVNULL)
        children.append(xserver)
        wait_for(lambda: Path(f"/tmp/.X11-unix/X{number}").exists(), "owned Xwayland socket")
        env["DISPLAY"] = display
        subprocess.run(["cargo", "test", "--locked", "--offline", "--lib", "native_layer_shell_unavailable_is_an_error_without_toplevel_fallback", "--", "--ignored", "--nocapture"], env=env, check=True, timeout=90)
        print("PASS: Weston without layer-shell rejects panels and still creates ordinary windows")
        (root / "config.toml").write_text("[dpms]\nenabled = false\n[layout]\nouter_gap = 0\ninner_gap = 0\nborder_width = 0\n")
        fraym_env = env.copy()
        fraym_env.pop("WAYLAND_DISPLAY")
        fraym_env.update(FRAYM_CONFIG=str(root / "config.toml"), FRAYM_SOCKET=str(root / "ipc.sock"), RUST_LOG="fraym=debug", WINIT_X11_SCALE_FACTOR="1")
        if args.labwc:
            fraym_env.update(WLR_BACKENDS="x11", WLR_RENDERER="pixman", WLR_X11_OUTPUTS="2" if args.remove_output else "1")
        with (root / "fraym.log").open("w") as log:
            command = [str(args.fraym), "--nested"] if args.fraym else [str(args.labwc), "-C", str(root), "-d"]
            fraym = subprocess.Popen(command, env=fraym_env, stdout=log, stderr=subprocess.STDOUT)
        children.append(fraym)
        if args.fraym:
            wait_for(lambda: (root / "ipc.sock").exists(), "owned FRAYM IPC socket")
        child_socket = wait_for(lambda: re.search(r"(?:WAYLAND_DISPLAY=|Socket Wayland: )(\S+)", (root / "fraym.log").read_text()), "compositor client socket")[1]
        assert child_socket != "wayland-0", "Do not use a regular-desktop socket name"
        env["WAYLAND_DISPLAY"] = child_socket
        if args.fraym:
            outputs = ctl(root / "ipc.sock", "outputs")["outputs"]
            assert len(outputs) == 1
            env["RUTTER_LAYER_OUTPUT"] = outputs[0]["name"]
        else:
            env["RUTTER_LAYER_OUTPUT"] = "X11-1"
            env["RUTTER_LAYER_MAXIMIZE"] = "1"
        env["WAYLAND_DEBUG"] = "client"
        if args.retained_layer or args.retained_renderer:
            if args.retained_layer:
                fixture = "retained_layer_output_removal_retires_native_lifetime_and_children" if args.remove_output else "retained_layer_closed_callback_retires_native_lifetime_and_children"
                command = ["cargo", "test", "--manifest-path", "vendor/rutter-winit/Cargo.toml", "-p", "winit-wayland"]
                env["CARGO_TARGET_DIR"] = "target"
            else:
                fixture = "native_layer_retained_opengl_renderer_rejects_frames_after_output_removal"
                command = ["cargo", "test"]
            with (root / "client.log").open("w") as log:
                client = subprocess.Popen(command + ["--locked", "--offline", "--lib", fixture, "--", "--ignored", "--nocapture"], env=env, stdout=log, stderr=subprocess.STDOUT)
            children.append(client)
            wait_for(lambda: "RETAINED_LAYER_READY" in (root / "client.log").read_text(), "retained layer fixture ready")
            if args.remove_output:
                tool_env = env.copy()
                tool_env.pop("WAYLAND_DEBUG", None)
                subprocess.run([str(args.wlr_randr), "--output", "X11-1", "--off"], env=tool_env, check=True, timeout=5)
            client.wait(timeout=20)
            trace = (root / "client.log").read_text()
            assert client.returncode == 0, trace
            assert "RETAINED_LAYER_RETIRED" in trace, trace
            assert re.search(r"zwlr_layer_surface_v1[#@]\d+\.destroy\(\)", trace)
            print("PASS: retained consumer terminal lifetime and cleanup", fixture)
            success = True
            return
        host = HostInput(display)
        host.window = wait_for(lambda: host_window(host.display, display, fraym.pid, bool(args.labwc)), "owned compositor X11 host window")
        host.x.XResizeWindow(host.display, host.window, 1100, 800)
        host.x.XFlush(host.display)
        time.sleep(0.3)
        env["RUTTER_LAYER_LIVE_INPUT"] = "1"
        if args.remove_output:
            env["RUTTER_LAYER_REMOVE_OUTPUT"] = "1"
        with (root / "client.log").open("w") as log:
            client = subprocess.Popen(["cargo", "test", "--locked", "--offline", "--lib", "native_layer_shell_panels_settings_and_popup_share_runner", "--", "--ignored", "--nocapture"], env=env, stdout=log, stderr=subprocess.STDOUT)
        children.append(client)
        wait_for(lambda: "LAYER_READY" in (root / "client.log").read_text(), "rendered panels and Settings")
        if args.fraym:
            before = ctl(root / "ipc.sock")
            print("FRAYM reserved scene", json.dumps(before), flush=True)
            settings = next(window for window in before["windows"] if window.get("title") == "Rutter layer Settings")
            assert len(before["windows"]) == 1, "Panels must not appear as ordinary toplevels"
            assert settings["geometry"]["y"] >= 40, settings
            focused_before = before.get("focused_window")
        else:
            time.sleep(0.3)
            colors = host.pixels([(550, 20), (550, 785), (15, 400), (1085, 400), (100, 785), (1085, 650), (550, 60)])
            print("Compositor pixel probes", [hex(c) for c in colors], flush=True)
            assert colors[:4] == [0xff0000, 0x00ff00, 0x0000ff, 0xff00ff], [hex(c) for c in colors]
            assert colors[4] != 0x00ff00 and colors[5] != 0xff00ff, "Compact panels must not stretch"
            if not args.remove_output:
                assert colors[6] == 0x888888, "Maximized Settings must begin below top reservation"
            trace = (root / "client.log").read_text()
            if not args.remove_output:
                assert re.search(r"LAYER_METRICS 5 .*logical_size: \(1100\.0, 760\.0\)", trace), "Reserved work area must configure Settings at output height minus zone"
            print("PASS: compositor pixels show four edge anchors, compact lengths and Settings below exclusive reservation")
        if args.remove_output:
            tool_env = env.copy()
            tool_env.pop("WAYLAND_DEBUG", None)
            subprocess.run([str(args.wlr_randr), "--output", "X11-1", "--off"], env=tool_env, check=True, timeout=5)
            wait_for(lambda: "LAYER_OUTPUT_REMOVED" in (root / "client.log").read_text(), "targeted panels retired after output destruction")
            client.wait(timeout=10)
            assert client.returncode == 0, (root / "client.log").read_text()
            trace = (root / "client.log").read_text()
            assert ".global_remove(" in trace and ".destroy()" in trace
            assert len(re.findall(r"get_layer_surface\(", trace)) <= 5, "Panels must not be recreated on replacement output"
            print("PASS: output disable removes wl_output, closes all targeted panels, retires routes, preserves Settings and never reattaches panels")
            success = True
            return
        else:
            if args.labwc:
                offset = len((root / "client.log").read_text())
                host.x.XResizeWindow(host.display, host.window, 900, 700)
                host.x.XFlush(host.display)
                wait_for(lambda: re.search(r"LAYER_METRICS 1 .*logical_size: \(900\.0, 40\.0\)", (root / "client.log").read_text()[offset:]), "stretched panel reconfigured after output resize")
                offset = len((root / "client.log").read_text())
                host.x.XResizeWindow(host.display, host.window, 1100, 800)
                host.x.XFlush(host.display)
                wait_for(lambda: re.search(r"LAYER_METRICS 1 .*logical_size: \(1100\.0, 40\.0\)", (root / "client.log").read_text()[offset:]), "restored output size")
                print("PASS: output resize updates configured stretched length and source-local metrics")
                if args.wlr_randr:
                    for scale in (2, 1.5, 1):
                        offset = len((root / "client.log").read_text())
                        tool_env = env.copy()
                        tool_env.pop("WAYLAND_DEBUG", None)
                        subprocess.run([str(args.wlr_randr), "--output", "X11-1", "--scale", str(scale)], env=tool_env, check=True, timeout=5)
                        wait_for(lambda: re.search(rf"LAYER_METRICS 1 .*scale_factor: {float(scale)}\b", (root / "client.log").read_text()[offset:]), f"configured layer scale {scale}")
                    print("PASS: integer/fractional output scale changes reach per-surface application hooks")
                time.sleep(0.25)
            host.click(20, 20, 3)
            wait_for(lambda: "LAYER_POPUP_READY" in (root / "client.log").read_text(), "mapped layer-owned popup")
            # Requested anchor at (20,20); the row extends below the 40px panel.
            host.click(40, 42, 1)
        wait_for(lambda: "LAYER_CLOSED" in (root / "client.log").read_text(), "menu selection and top-panel destruction")
        if args.fraym:
            after = wait_for(lambda: (snapshot if (snapshot := ctl(root / "ipc.sock"))["windows"] and snapshot["windows"][0]["geometry"]["y"] < settings["geometry"]["y"] else None), "restored Settings work area", timeout=1.8)
            restored = after["windows"][0]["geometry"]
            assert restored["y"] == 0 and restored["height"] > settings["geometry"]["height"], (before, after)
            assert after.get("focused_window") == focused_before, "Noninteractive layer and menu must preserve ordinary application focus"
        else:
            wait_for(lambda: re.search(r"LAYER_METRICS 5 .*logical_size: \(1100\.0, 800\.0\)", (root / "client.log").read_text()), "restored Settings height", timeout=1.8)
            time.sleep(0.15)
            assert host.pixels([(550, 20)])[0] == 0x888888
        client.wait(timeout=30)
        assert client.returncode == 0, (root / "client.log").read_text()
        validate_protocol((root / "client.log").read_text())
        print("PASS: four edge panels, compact lengths, stable output targeting, Settings work-area reservation/restoration, popup selection and preserved Settings focus")
        success = True
    finally:
        if host:
            host.close()
        for child in reversed(children):
            stop(child)
        socket_zero_lock.close()
        print(f"Logs: {root}")
        if success and not args.keep:
            import shutil
            shutil.rmtree(root)


if __name__ == "__main__":
    main()
