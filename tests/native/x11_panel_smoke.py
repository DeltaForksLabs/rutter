"""Owned Weston/Xwayland/Openbox panel regression; never uses a desktop display.

Build first: CARGO_BUILD_JOBS=2 cargo test --locked --offline --lib --no-run
Run: python3 tests/native/x11_panel_smoke.py --openbox /absolute/path/to/openbox
An extracted Openbox may need its library directory in LD_LIBRARY_PATH.
"""
import argparse
import ctypes as C
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

from layer_shell_smoke import HostInput, stop, wait_for


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--openbox", type=Path, required=True)
    args = parser.parse_args()
    assert args.openbox.is_absolute() and os.access(args.openbox, os.X_OK)
    with tempfile.TemporaryDirectory(prefix="rutter-x11-panel-", dir="/tmp/opencode") as temporary:
        directory = Path(temporary)
        excluded = {"DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "XAUTHORITY", "SESSION_MANAGER", "DESKTOP_AUTOSTART_ID", "XDG_SESSION_ID", "XDG_SESSION_TYPE", "XDG_SESSION_DESKTOP", "XDG_CURRENT_DESKTOP", "DESKTOP_SESSION", "GNOME_DESKTOP_SESSION_ID", "KDE_FULL_SESSION", "WAYLAND_DEBUG"}
        env = {key: value for key, value in os.environ.items() if key not in excluded and not key.startswith(("RUTTER_", "DBUS_"))}
        env.update(XDG_RUNTIME_DIR=temporary, XDG_CONFIG_HOME=temporary, XDG_CONFIG_DIRS=temporary, LIBGL_ALWAYS_SOFTWARE="1", CARGO_BUILD_JOBS="2", WINIT_X11_SCALE_FACTOR="1", RUTTER_X11_PANEL_OWNED="1", RUTTER_X11_PANEL_DIRECTORY=temporary)
        env["XDG_DATA_DIRS"] = str(args.openbox.parent.parent / "share") + ":/usr/share"
        children = []
        host = None
        logs = []
        try:
            weston = subprocess.Popen(["weston", "--backend=headless", "--renderer=pixman", "--width=800", "--height=600", "--socket=rutter-panel-host", "--no-config", f"--log={directory / 'weston.log'}"], env=env)
            children.append(weston)
            wait_for(lambda: (directory / "rutter-panel-host").exists(), "private Weston socket")
            server_env = env | {"WAYLAND_DISPLAY": "rutter-panel-host"}
            number = next(number for number in range(98, 120) if not Path(f"/tmp/.X11-unix/X{number}").exists() and not Path(f"/tmp/.X{number}-lock").exists())
            display = f":{number}"
            xserver = subprocess.Popen(["Xwayland", display, "-shm", "-ac", "-noreset", "-geometry", "800x600"], env=server_env, stdout=subprocess.DEVNULL)
            children.append(xserver)
            wait_for(lambda: Path(f"/tmp/.X11-unix/X{number}").exists(), "private Xwayland socket")
            env["DISPLAY"] = display
            subprocess.run(["cargo", "test", "--locked", "--offline", "--lib", "native_x11_panel_requires_ewmh_without_toplevel_fallback", "--", "--ignored", "--nocapture"], env=env, check=True, timeout=120)
            subprocess.run(["cargo", "test", "--manifest-path", "vendor/rutter-winit/Cargo.toml", "-p", "winit-x11", "--test", "popup_keyboard_focus", "--locked", "--offline", "--", "--ignored", "--nocapture"], env=env | {"CARGO_TARGET_DIR": "target"}, check=True, timeout=120)
            # No session wrapper or autostart script: start only this owned WM.
            config = directory / "openbox.xml"
            config.write_text('<openbox_config xmlns="http://openbox.org/3.4/rc"><focus><focusNew>no</focusNew></focus><desktops><number>1</number></desktops></openbox_config>')
            log = (directory / "openbox.log").open("w")
            logs.append(log)
            wm = subprocess.Popen([str(args.openbox), "--sm-disable", "--config-file", str(config)], env=env, stdout=log, stderr=subprocess.STDOUT)
            children.append(wm)

            def prop(window, name):
                return subprocess.run(["xprop", "-display", display, "-root" if window is None else "-id", *([] if window is None else [str(window)]), name], env=env, check=True, capture_output=True, text=True, timeout=3).stdout

            wait_for(lambda: "window id" in prop(None, "_NET_SUPPORTING_WM_CHECK"), "owned EWMH WM")
            subprocess.run(["cargo", "test", "--manifest-path", "vendor/rutter-winit/Cargo.toml", "-p", "winit-x11", "--lib", "terminal_focus_and_late_ancestor_destroy_do_not_dispatch", "--locked", "--offline", "--", "--ignored", "--nocapture"], env=env | {"CARGO_TARGET_DIR": "target"}, check=True, timeout=120)
            host = HostInput(display)
            host.x.XGetInputFocus.argtypes = [C.c_void_p, C.POINTER(C.c_ulong), C.POINTER(C.c_int)]

            def focus():
                window, revert = C.c_ulong(), C.c_int()
                host.x.XGetInputFocus(host.display, C.byref(window), C.byref(revert))
                return window.value

            def geometry(window):
                text = subprocess.run(["xwininfo", "-display", display, "-id", str(window)], env=env, check=True, capture_output=True, text=True, timeout=3).stdout
                values = [int(re.search(label + r"\s*:\s*(-?\d+)", text)[1]) for label in ["Absolute upper-left X", "Absolute upper-left Y", "Width", "Height"]]
                return values, "IsViewable" in text

            def workarea(height):
                text = prop(None, "_NET_WORKAREA")
                return re.search(r"=\s*0,\s*" + ("40" if height == 560 else "0") + r",\s*800,\s*" + str(height) + r"\s*$", text)

            test_log = (directory / "test.log").open("w")
            logs.append(test_log)
            test = subprocess.Popen(["cargo", "test", "--locked", "--offline", "--lib", "native_x11_panel_managed_reservation_focus_and_retained_renderer", "--", "--ignored", "--nocapture"], env=env, stdout=test_log, stderr=subprocess.STDOUT)
            children.append(test)

            def stage(expected):
                def reached():
                    if (directory / "stage").exists() and (directory / "stage").read_text() == expected:
                        return True
                    if test.poll() is not None:
                        raise RuntimeError((directory / "test.log").read_text())
                    return False
                wait_for(reached, expected, timeout=45)
                time.sleep(0.15)

            def command(value, expected):
                # Atomic command publication prevents partial command reads.
                path = directory / "pending"
                path.write_text(value)
                path.replace(directory / "command")
                stage(expected)

            stage("ready")
            top, right, bottom, left, settings = map(int, (directory / "windows").read_text().split())
            for window, expected in [(top, [0, 0, 800, 40]), (right, [760, 200, 40, 200]), (bottom, [200, 560, 400, 40]), (left, [0, 200, 40, 200])]:
                assert "_NET_WM_WINDOW_TYPE_DOCK" in prop(window, "_NET_WM_WINDOW_TYPE")
                assert "input or input focus: False" in prop(window, "WM_HINTS")
                actual, mapped = geometry(window)
                assert mapped and actual == expected, (actual, expected)
                attributes = subprocess.run(["xwininfo", "-display", display, "-id", str(window), "-stats"], env=env, check=True, capture_output=True, text=True).stdout
                assert "Override Redirect State: no" in attributes
            assert re.search(r"=\s*0, 0, 40, 0, 0, 0, 0, 0, 0, 799, 0, 0", prop(top, "_NET_WM_STRUT_PARTIAL"))
            wait_for(lambda: workarea(560), "reserved work area")
            wait_for(lambda: focus() == settings, "Settings focus after nonactivating panels")
            maximized, _ = geometry(settings)
            assert maximized[1] >= 40 and maximized[1] + maximized[3] <= 600, maximized
            command("popup", "popup")
            popup = int((directory / "popup").read_text())
            assert geometry(popup)[1]
            assert focus() == settings, "nonactivating panel-owned popup stole Settings focus"
            command("active", "active")
            wait_for(lambda: focus() == int((directory / "popup").read_text()), "active panel menu focus")
            command("dismiss", "dismissed")
            wait_for(lambda: focus() == settings, "previous application focus restored after active menu")
            host.x.XDefaultRootWindow.argtypes = [C.c_void_p]
            host.x.XDefaultRootWindow.restype = C.c_ulong
            host.x.XCreateSimpleWindow.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_uint, C.c_ulong, C.c_ulong]
            host.x.XCreateSimpleWindow.restype = C.c_ulong
            host.x.XMapWindow.argtypes = [C.c_void_p, C.c_ulong]
            external = host.x.XCreateSimpleWindow(host.display, host.x.XDefaultRootWindow(host.display), 80, 80, 120, 80, 0, 0, 0)
            host.x.XMapWindow(host.display, external)
            host.x.XFlush(host.display)
            wait_for(lambda: geometry(external)[1], "separate-process focus target mapped")
            host.x.XSetInputFocus(host.display, external, 2, 0)
            host.x.XFlush(host.display)
            wait_for(lambda: focus() == external, "separate-process focus target focused")
            command("immediate-active", "immediate-dismissed")
            wait_for(lambda: focus() == external, "immediate popup close restores external application before FocusIn drain")
            command("active", "active")
            wait_for(lambda: focus() == int((directory / "popup").read_text()), "panel menu focuses over external application")
            command("dismiss", "dismissed")
            wait_for(lambda: focus() == external, "panel menu restores separate-process application focus")
            host.x.XSetInputFocus(host.display, settings, 2, 0)
            host.x.XFlush(host.display)
            host.x.XDestroyWindow(host.display, external)
            host.x.XFlush(host.display)
            wait_for(lambda: focus() == settings, "Settings focus restored for remaining probes")
            command("hide", "hidden")
            assert not geometry(top)[1]
            wait_for(lambda: workarea(600), "reservation release on hide")
            assert "not found" in prop(top, "_NET_WM_STRUT_PARTIAL")
            command("show", "shown")
            wait_for(lambda: workarea(560), "reservation restoration on show")
            assert focus() == settings
            command("quick-show", "quick-shown")
            wait_for(lambda: geometry(top)[1] and workarea(560), "quick hide/show retains mapping and reservation")
            command("quick-hide", "quick-hidden")
            wait_for(lambda: not geometry(top)[1] and workarea(600), "quick show/hide releases mapping and reservation")
            assert "not found" in prop(top, "_NET_WM_STRUT_PARTIAL")
            command("show", "shown")
            wait_for(lambda: geometry(top)[1] and workarea(560), "show after queued visibility notifications")
            subprocess.run(["xrandr", "--display", display, "--size", "640x480"], env=env, check=True, capture_output=True, text=True)
            wait_for(lambda: geometry(top)[0] == [0, 0, 640, 40], "stretch panel recomputed from full resized output")
            wait_for(lambda: "639" in prop(top, "_NET_WM_STRUT_PARTIAL"), "partial strut updated on root resize")
            subprocess.run(["xrandr", "--display", display, "--size", "800x600"], env=env, check=True, capture_output=True, text=True)
            wait_for(lambda: geometry(top)[0] == [0, 0, 800, 40], "stretch panel restored after root resize")
            command("close", "closed")
            wait_for(lambda: workarea(600), "reservation release on close")
            command("retire", "retire-ready")
            wait_for(lambda: focus() == int((directory / "nested-popup").read_text()), "active nested popup focus before retirement")
            # A smaller full root makes the compact bottom panel impossible.
            # This exercises terminal geometry loss without destroying the EGL drawable.
            resize = subprocess.run(["xrandr", "--display", display, "--size", "320x240"], env=env, capture_output=True, text=True)
            assert resize.returncode == 0, resize.stderr
            stage("retired")
            assert not geometry(int((directory / "popup").read_text()))[1]
            command("drop-retired-ancestor", "ancestor-dropped")
            time.sleep(0.2)  # Drain late ancestor DestroyNotify with its descendant retained.
            wait_for(lambda: not geometry(bottom)[1], "terminal panel withdrawal")
            assert subprocess.run(["xwininfo", "-display", display, "-id", (directory / "popup").read_text()], env=env, capture_output=True, timeout=3).returncode != 0, "retired ancestor not dropped"
            assert subprocess.run(["xwininfo", "-display", display, "-id", (directory / "nested-popup").read_text()], env=env, capture_output=True, timeout=3).returncode != 0, "nested renderer/window not dropped during terminal dispatch"
            wait_for(lambda: geometry(right)[0] == [280, 20, 40, 200], "panel reposition after root resize")
            subprocess.run(["xrandr", "--display", display, "--size", "800x600"], env=env, check=True, capture_output=True, text=True)
            wait_for(lambda: geometry(right)[0] == [760, 200, 40, 200], "surviving panel restored geometry")
            assert not geometry(bottom)[1], "retired panel remapped on restored output geometry"
            command("finish", "finished")
            assert test.wait(timeout=15) == 0, (directory / "test.log").read_text()
            print("PASS: managed DOCK geometry/struts/workarea, rapid hide/show reconciliation, malformed EWMH rejection, popup focus, atomic nested retirement/reentrant creation rejection, queued terminal input suppression and retained renderer cleanup")
        except Exception as error:
            if isinstance(error, subprocess.CalledProcessError):
                print(f"Native probe command failed: {error.cmd}\n{error.stdout or ''}{error.stderr or ''}")
            for path in [directory / "test.log", directory / "openbox.log"]:
                if path.exists():
                    print(path.read_text())
            raise
        finally:
            if host:
                host.close()
            for child in reversed(children):
                stop(child)
            for log in logs:
                log.close()


if __name__ == "__main__":
    main()
