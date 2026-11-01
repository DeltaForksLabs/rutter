"""Driver contract tests; no displays, compositor, Cargo or native fixture started."""

import os
from pathlib import Path
import signal
import subprocess
import unittest
from unittest.mock import Mock, patch

import minimized_surface_smoke as smoke


class MinimizedSurfaceDriverTests(unittest.TestCase):
    def test_only_completed_exact_sequence_is_progress(self):
        trace = "MINIMIZED_READY OpenGl\nMINIMIZED_INPUT 1 a\nMINIMIZED_DONE 10 9 1 a\n"
        self.assertIsNone(smoke.completed(trace, 1))
        self.assertEqual(smoke.completed(trace, 10), (9, 1, "a"))
        self.assertEqual(smoke.completed("MINIMIZED_DONE 2 0 0 -\n", 2), (0, 0, ""))
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            smoke.completed(trace + "MINIMIZED_DONE 10 9 1 a\n", 10)

    def test_settings_state_requires_same_identity_and_actual_minimized_bit(self):
        window = {"id": 7, "title": smoke.TITLE, "minimized": True, "geometry": None}
        self.assertEqual(smoke.settings_window({"windows": [window]}, 7, True), window)
        self.assertIsNone(smoke.settings_window({"windows": [window]}, 8, True))
        self.assertIsNone(smoke.settings_window({"windows": [window]}, 7, False))
        window["minimized"] = False
        self.assertIsNone(smoke.settings_window({"windows": [window]}, 7, False))
        window["geometry"] = {"x": 0, "y": 64, "width": 640, "height": 360}
        self.assertEqual(smoke.settings_window({"windows": [window]}, 7, False), window)
        self.assertIsNone(smoke.settings_window({"windows": [window, dict(window)]}, 7, False))

    def test_private_environment_drops_desktop_and_compositor_handles(self):
        inherited = {"DISPLAY": ":0", "WAYLAND_DISPLAY": "host", "WAYLAND_SOCKET": "8",
                     "XAUTHORITY": "/host/auth", "DBUS_OTHER": "host", "FRAYM_SOCKET": "/host/ipc",
                     "WLR_BACKENDS": "drm", "LD_PRELOAD": "/host/hook", "PATH": "/usr/bin"}
        with patch.dict(os.environ, inherited, clear=True):
            env = smoke.private_environment(Path("/private"))
        for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DBUS_OTHER",
                    "FRAYM_SOCKET", "WLR_BACKENDS", "LD_PRELOAD"):
            self.assertNotIn(key, env)
        self.assertEqual(env["XAUTHORITY"], "/private/Xauthority")
        self.assertEqual(env["XDG_RUNTIME_DIR"], "/private/runtime")
        self.assertEqual(env["PATH"], "/usr/bin")

    def test_protocol_rejects_missing_native_role_or_backend(self):
        trace = ("MINIMIZED_READY OpenGl\n"
                 "get_layer_surface(new id zwlr_layer_surface_v1@3, wl_surface@4, nil)\n"
                 "get_xdg_surface(new id xdg_surface@6, wl_surface@7)\n"
                 "get_toplevel(new id xdg_toplevel@5)\n"
                 "wl_surface@4.frame(callback)\nwl_surface@4.attach(buffer)\n"
                 "wl_surface@7.frame(callback)\nwl_surface@7.attach(buffer)\n")
        smoke.validate_protocol(trace)
        for invalid in (trace.replace("OpenGl", "CpuSoftbuffer"),
                        trace.replace("wl_surface@4.attach(buffer)", ""),
                        trace.replace("wl_surface@4.frame(callback)", ""),
                        trace.replace("wl_surface@7.frame(callback)\nwl_surface@7.attach(buffer)",
                                      "wl_surface@7.attach(buffer)\nwl_surface@7.frame(callback)"),
                        trace + "get_toplevel(new id xdg_toplevel@6)\n"):
            with self.assertRaises(AssertionError):
                smoke.validate_protocol(invalid)

    @patch.object(smoke.os, "killpg")
    @patch.object(smoke.subprocess, "Popen")
    def test_external_watchdog_kills_owned_group_on_timeout(self, popen, killpg):
        process = Mock(pid=123)
        process.wait.side_effect = [subprocess.TimeoutExpired("worker", 2), -9]
        popen.return_value = process
        with self.assertRaisesRegex(RuntimeError, "watchdog expired"):
            smoke.supervise(["worker"], 2)
        popen.assert_called_once_with(["worker"], start_new_session=True)
        killpg.assert_called_once_with(123, signal.SIGKILL)
        self.assertEqual(process.wait.call_count, 2)

    @patch.object(smoke.os, "killpg", side_effect=ProcessLookupError)
    @patch.object(smoke.subprocess, "Popen")
    def test_success_still_cleans_descendants_and_reaps_worker(self, popen, killpg):
        process = Mock(pid=456)
        process.wait.return_value = 0
        popen.return_value = process
        self.assertEqual(smoke.supervise(["worker"], 2), 0)
        killpg.assert_called_once_with(456, signal.SIGKILL)
        self.assertEqual(process.wait.call_count, 2)


if __name__ == "__main__":
    unittest.main()
