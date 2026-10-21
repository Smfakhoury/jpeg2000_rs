import subprocess
import sys
import threading
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import framework


class ProcessBoundsTests(unittest.TestCase):
    def runners(self):
        return (framework._run_process_bounded_threaded,
                lambda command, timeout, bound: framework._run_process_bounded(command, timeout, bound, None))

    def test_both_streams_are_drained_and_binary_output_and_exit_status_preserved(self):
        script = ("import sys; data=bytes(range(256))*1024; "
                  "sys.stdout.buffer.write(data); sys.stdout.flush(); "
                  "sys.stderr.buffer.write(data[::-1]); sys.stderr.flush(); sys.exit(7)")
        for runner in self.runners():
            with self.subTest(runner=runner):
                process, elapsed, limited = runner([sys.executable, "-c", script], 10.0, 1 << 20)
                self.assertEqual(process.returncode, 7)
                self.assertEqual(process.stdout, bytes(range(256)) * 1024)
                self.assertEqual(process.stderr, (bytes(range(256)) * 1024)[::-1])
                self.assertFalse(limited)
                self.assertGreater(elapsed, 0)

    def test_combined_output_bound_stops_the_child_and_capture_threads(self):
        script = ("import sys\nwhile True:\n"
                  " sys.stdout.buffer.write(b'a'*16384); sys.stdout.flush()\n"
                  " sys.stderr.buffer.write(b'b'*16384); sys.stderr.flush()\n")
        original = subprocess.Popen
        for runner in self.runners():
            children = []

            def start(*args, **kwargs):
                child = original(*args, **kwargs)
                children.append(child)
                return child

            with self.subTest(runner=runner), mock.patch.object(framework.subprocess, "Popen", side_effect=start):
                process, _, limited = runner([sys.executable, "-c", script], 10.0, 8192)
            self.assertTrue(limited)
            self.assertLessEqual(len(process.stdout) + len(process.stderr), 8192)
            self.assertTrue(all(child.poll() is not None for child in children))
            self.assertFalse(any(thread.name.startswith("jpeg2000-capture-")
                                 for thread in threading.enumerate()))
            finite = ("import sys,time; sys.stdout.buffer.write(b'a'*6000); sys.stdout.flush(); "
                      "sys.stderr.buffer.write(b'b'*6000); sys.stderr.flush(); time.sleep(30)")
            process, _, limited = runner([sys.executable, "-c", finite], 10.0, 8192)
            self.assertTrue(limited)
            self.assertLessEqual(len(process.stdout) + len(process.stderr), 8192)
            self.assertNotEqual(process.returncode, None)

    def test_deadline_stops_the_child_and_pipe_readers(self):
        original = subprocess.Popen
        for runner in self.runners():
            children = []

            def start(*args, **kwargs):
                child = original(*args, **kwargs)
                children.append(child)
                return child

            with self.subTest(runner=runner), mock.patch.object(framework.subprocess, "Popen", side_effect=start):
                for script in ("import time; time.sleep(30)",
                               "import os,time; os.close(1); os.close(2); time.sleep(30)"):
                    with self.assertRaises(subprocess.TimeoutExpired):
                        runner([sys.executable, "-c", script], 0.1, 1024)
            self.assertTrue(all(child.poll() is not None for child in children))
            self.assertFalse(any(thread.name.startswith("jpeg2000-capture-")
                                 for thread in threading.enumerate()))

    def test_windows_dispatch_does_not_silently_drop_posix_setup(self):
        with mock.patch.object(framework.sys, "platform", "win32"), \
                mock.patch.object(framework.subprocess, "Popen") as start:
            with self.assertRaisesRegex(ValueError, "POSIX process setup"):
                framework._run_process_bounded(["not-run"], 1.0, 1024, lambda: None)
            start.assert_not_called()
