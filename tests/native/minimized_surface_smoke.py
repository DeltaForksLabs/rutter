"""Bounded minimized-Settings/shared-OpenGL regression on owned Xvfb + FRAYM.

Build the library test executable separately; --test-binary never invokes Cargo.
See MINIMIZED_SURFACE.md for exact commands, dependencies and evidence limits.
"""

import argparse
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

from layer_shell_smoke import host_window, stop, wait_for
from native_popup_smoke import HEIGHT, WIDTH, PopupInput


TEST = "engine::multi_runner::minimized_integration::native_minimized_settings_does_not_stall_shared_gl_runtime"
TITLE = "Rutter minimized Settings"


def private_environment(root):
    excluded = {
        "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "WAYLAND_DEBUG", "XAUTHORITY",
        "SESSION_MANAGER", "XDG_SESSION_ID", "XDG_SESSION_TYPE", "XDG_SESSION_DESKTOP",
        "XDG_CURRENT_DESKTOP", "DESKTOP_SESSION", "DESKTOP_AUTOSTART_ID", "LD_PRELOAD",
        "WINIT_UNIX_BACKEND", "RUTTER_MINIMIZED_COMMAND",
    }
    env = {key: value for key, value in os.environ.items()
           if key not in excluded and not key.startswith(("DBUS_", "FRAYM_", "WLR_"))}
    env.update(
        HOME=str(root / "home"), XDG_CONFIG_HOME=str(root / "home"),
        XDG_CONFIG_DIRS=str(root / "home"), XDG_DATA_HOME=str(root / "home"),
        XDG_DATA_DIRS=str(root / "home"), XDG_CACHE_HOME=str(root / "home"),
        XDG_RUNTIME_DIR=str(root / "runtime"), XAUTHORITY=str(root / "Xauthority"),
        LIBGL_ALWAYS_SOFTWARE="1", WINIT_X11_SCALE_FACTOR="1",
        DBUS_SESSION_BUS_ADDRESS=f"unix:path={root}/no-session-bus",
    )
    return env


def ipc(path, request="state", arguments=None):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(2)
        connection.connect(str(path))
        connection.sendall((json.dumps({"version": 1, "request": request,
                                       "args": arguments or {}}) + "\n").encode())
        with connection.makefile("rb") as reader:
            line = reader.readline(1024 * 1024)
    reply = json.loads(line)
    if reply.get("ok") is not True:
        raise RuntimeError(f"FRAYM {request} failed: {reply}")
    return reply


def settings_window(state, window_id=None, minimized=None):
    matches = [window for window in state["windows"]
               if window.get("title") == TITLE and (window_id is None or window["id"] == window_id)]
    if len(matches) != 1:
        return None
    window = matches[0]
    if minimized is not None and window.get("minimized") is not minimized:
        return None
    if minimized is False and not window.get("geometry"):
        return None
    return window


def completed(trace, sequence):
    matches = re.findall(rf"^MINIMIZED_DONE {sequence} (\d+) (\d+) (\S+)\s*$", trace, re.M)
    if not matches:
        return None
    if len(matches) != 1:
        raise RuntimeError(f"duplicate completion marker for command {sequence}")
    publication, clicks, draft = matches[0]
    return int(publication), int(clicks), "" if draft == "-" else draft


def validate_protocol(trace):
    assert len(re.findall(r"get_layer_surface\(new id zwlr_layer_surface_v1", trace)) == 1, "expected one actual layer role"
    assert len(re.findall(r"get_toplevel\(new id xdg_toplevel", trace)) == 1, "expected one ordinary Settings toplevel"
    assert "MINIMIZED_READY OpenGl" in trace, "fixture must require OpenGL without fallback"
    roles = re.findall(r"get_layer_surface\(new id zwlr_layer_surface_v1[#@]\d+, wl_surface[#@](\d+)", trace)
    assert roles and re.search(rf"wl_surface[#@]{roles[0]}\.attach\(", trace), "panel never submitted a native buffer"
    settings = re.findall(r"get_xdg_surface\(new id xdg_surface[#@]\d+, wl_surface[#@](\d+)\)", trace)
    assert len(settings) == 1, "expected one ordinary xdg surface"
    for surface in (roles[0], settings[0]):
        callback = re.search(rf"wl_surface[#@]{surface}\.frame\(", trace)
        attachment = re.search(rf"wl_surface[#@]{surface}\.attach\(", trace)
        assert callback and attachment and callback.start() < attachment.start(), "request frame callback before first buffer submission"
    assert "error in client communication" not in trace.lower()


class Fixture:
    def __init__(self, root, process):
        self.root, self.process, self.sequence = root, process, 0

    def trace(self):
        return (self.root / "fixture.log").read_text(errors="replace")

    def native_input(self, clicks, draft):
        marker = f"MINIMIZED_INPUT {clicks} {draft or '-'}"
        wait_for(lambda: marker in self.trace().splitlines(), "native input reaches canonical model", timeout=8)

    def command(self, operation):
        self.sequence += 1
        pending = self.root / "command.pending"
        pending.write_text(f"{self.sequence} {operation}\n")
        pending.replace(self.root / "command")

        def reply():
            result = completed(self.trace(), self.sequence)
            if result is None and self.process.poll() is not None:
                raise RuntimeError(f"fixture exited {self.process.returncode}: {self.trace()}")
            return result

        return wait_for(reply, f"completed redraw after {operation}", timeout=8)


def click(host, x, y):
    host.move(x, y)
    host.button(True)
    time.sleep(0.1)
    host.button(False)
    # Do not confuse hover or cursor changes with the model-owned color/text.
    host.move(WIDTH - 20, HEIGHT - 20)


def exercise(host, root, env, binary, launch):
    client_env = dict(env, WAYLAND_DISPLAY=env["WAYLAND_DISPLAY"],
                      WAYLAND_DEBUG="client", RUTTER_MINIMIZED_COMMAND=str(root / "command"))
    client = launch("fixture", [binary, "--exact", TEST, "--ignored", "--nocapture", "--test-threads=1"], client_env)
    fixture = Fixture(root, client)
    wait_for(lambda: "MINIMIZED_READY OpenGl" in fixture.trace(), "both actual GL surfaces redraw", timeout=20)
    ipc_path = root / "fraym.sock"
    window = wait_for(lambda: settings_window(ipc(ipc_path), minimized=False), "ordinary Settings mapped")
    window_id = window["id"]
    publication, clicks, draft = 0, 0, ""

    def require_minimized():
        assert settings_window(ipc(ipc_path), window_id, True), "Settings unexpectedly restored/recreated during minimized work"

    def heading(geometry):
        return host.sample(geometry["x"] + 24, geometry["y"] + 24, 500, 30)

    for cycle in range(3):
        geometry = window["geometry"]
        old_heading = heading(geometry)
        # Native keyboard input exercises the controlled draft before each cycle.
        click(host, geometry["x"] + 100, geometry["y"] + 110)
        host.key("a")
        draft += "a"
        fixture.native_input(clicks, draft)
        assert fixture.command(f"check {publication} {clicks} {draft}") == (publication, clicks, draft)
        ipc(ipc_path, "minimize-window", {"id": window_id})
        wait_for(lambda: settings_window(ipc(ipc_path), window_id, True), "FRAYM confirms Settings minimized", timeout=8)
        (root / f"minimized-{cycle}.json").write_text(json.dumps(ipc(ipc_path), indent=2))

        for _ in range(3):
            require_minimized()
            publication += 1
            # ACK is after Settings' redraw returns, not before eglSwapBuffers.
            assert fixture.command(f"publish {publication}") == (publication, clicks, draft)
            require_minimized()
            before = host.sample(32, 16, 128, 24)
            click(host, 120, 32)
            clicks += 1
            fixture.native_input(clicks, draft)
            assert fixture.command(f"check {publication} {clicks} {draft}") == (publication, clicks, draft)
            wait_for(lambda: host.sample(32, 16, 128, 24) != before,
                     "visible panel activation pixels change while Settings minimized", timeout=4)
            require_minimized()

        ipc(ipc_path, "restore-window", {"id": window_id})
        window = wait_for(lambda: settings_window(ipc(ipc_path), window_id, False),
                          "same Settings restored with geometry", timeout=8)
        assert fixture.command(f"check {publication} {clicks} {draft}") == (publication, clicks, draft)
        wait_for(lambda: heading(window["geometry"]) != old_heading,
                 "restored Settings paints latest model heading", timeout=4)
        (root / f"restored-{cycle}.json").write_text(json.dumps(ipc(ipc_path), indent=2))
        print(f"PASS cycle {cycle + 1}: minimized publications, native panel input/pixels, same Settings/draft restored", flush=True)

    # A final edit proves the last restored input widget retained its full draft,
    # rather than merely retaining a canonical string never painted/used by input.
    geometry = window["geometry"]
    click(host, geometry["x"] + 100, geometry["y"] + 110)
    host.key("a")
    draft += "a"
    fixture.native_input(clicks, draft)
    assert fixture.command(f"check {publication} {clicks} {draft}") == (publication, clicks, draft)
    validate_protocol(fixture.trace())
    assert len(re.findall(r"wl_pointer[#@]\d+\.button\(", fixture.trace())) >= 18
    (root / "command.pending").write_text(f"{fixture.sequence + 1} quit\n")
    (root / "command.pending").replace(root / "command")
    assert client.wait(timeout=5) == 0, fixture.trace()
    assert "MINIMIZED_EXIT" in fixture.trace(), fixture.trace()


def worker(args, root):
    children, host = [], None
    for directory in (root / "home", root / "runtime"):
        directory.mkdir(mode=0o700)
    env = private_environment(root)

    def launch(name, command, environment):
        with (root / f"{name}.log").open("w") as log:
            process = subprocess.Popen([str(part) for part in command], env=environment,
                                       stdout=log, stderr=subprocess.STDOUT)
        children.append(process)
        return process

    try:
        number = next(n for n in range(98, 200) if not Path(f"/tmp/.X11-unix/X{n}").exists()
                      and not Path(f"/tmp/.X{n}-lock").exists())
        display = f":{number}"
        env["DISPLAY"] = display
        subprocess.run(["xauth", "-f", env["XAUTHORITY"], "add", display, ".", secrets.token_hex(16)],
                       env=env, check=True, timeout=5, stdout=subprocess.DEVNULL)
        launch("xvfb", [args.xvfb.resolve(), display, "-screen", "0", f"{WIDTH}x{HEIGHT}x24",
                        "-nolisten", "tcp", "-auth", env["XAUTHORITY"], "-noreset"], env)
        wait_for(lambda: Path(f"/tmp/.X11-unix/X{number}").exists(), "owned authenticated Xvfb")
        config = root / "fraym.toml"
        config.write_text("[dpms]\nenabled = false\n[layout]\nouter_gap = 0\ninner_gap = 0\nborder_width = 0\n")
        compositor_env = dict(env, FRAYM_CONFIG=str(config), FRAYM_SOCKET=str(root / "fraym.sock"))
        compositor = launch("fraym", [args.fraym.resolve(), "--nested"], compositor_env)
        wait_for(lambda: (root / "fraym.sock").is_socket(), "owned FRAYM IPC")
        wayland = wait_for(lambda: re.search(r"Socket Wayland: (\S+)", (root / "fraym.log").read_text(errors="replace")),
                           "owned FRAYM Wayland socket")[1]
        assert Path(wayland).name == wayland and (root / "runtime" / wayland).is_socket(), "socket must belong to private runtime"
        env["WAYLAND_DISPLAY"] = wayland
        os.environ["XAUTHORITY"] = env["XAUTHORITY"]
        host = PopupInput(display)
        # This authenticated private server has one compositor host client only.
        host.window = wait_for(lambda: host_window(host.display, display, compositor.pid, True), "owned compositor host window")
        host.x.XResizeWindow(host.display, host.window, WIDTH, HEIGHT)
        host.x.XRaiseWindow(host.display, host.window)
        host.x.XSetInputFocus(host.display, host.window, 2, 0)
        host.x.XFlush(host.display)
        wait_for(lambda: any(output["geometry"]["width"] == WIDTH and output["geometry"]["height"] == HEIGHT
                             and output["scale"] == 1 for output in ipc(root / "fraym.sock")["outputs"]),
                 "1x compositor output resized to private host")
        exercise(host, root, env, args.test_binary.resolve(), launch)
    finally:
        if host:
            host.close()
        for child in reversed(children):
            stop(child)


def supervise(command, timeout):
    """Independent parent deadline also kills children if Xlib or EGL wedges."""
    child = subprocess.Popen(command, start_new_session=True)
    try:
        try:
            return child.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            raise RuntimeError(f"external minimized-surface watchdog expired after {timeout}s") from error
    finally:
        # Every owned compositor/client inherits this new process group. Kill the
        # group even after worker exit, covering failed cleanup and descendants.
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fraym", type=Path, required=True)
    parser.add_argument("--xvfb", type=Path, required=True)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=150)
    parser.add_argument("--keep", action="store_true")
    parser.add_argument("--worker", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not 1 <= args.timeout <= 300:
        parser.error("--timeout must be between 1 and 300 seconds")
    for binary in (args.fraym, args.xvfb, args.test_binary):
        if not binary.is_file() or not os.access(binary, os.X_OK):
            parser.error(f"expected executable file: {binary}")
    if args.worker:
        worker(args, args.worker)
        return
    root = Path(tempfile.mkdtemp(prefix="rutter-minimized-", dir="/tmp/opencode"))
    success = False
    try:
        command = [sys.executable, "-u", str(Path(__file__).resolve()), *sys.argv[1:], "--worker", str(root)]
        result = supervise(command, args.timeout)
        if result:
            raise RuntimeError(f"minimized-surface worker exited {result}")
        success = True
    finally:
        if success and not args.keep:
            shutil.rmtree(root)
        else:
            print(f"Artifacts: {root}", flush=True)


if __name__ == "__main__":
    main()
