"""The E007 timeout must not erase executable/input identities."""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest


class CancellationCheckpoint(unittest.TestCase):
    @unittest.skipUnless(os.name == "posix", "hard cancellation uses POSIX signals")
    def test_hard_kill_preserves_running_identities(self):
        evaluator = Path(__file__).with_name("real_mix_eval.py").resolve()
        plan = evaluator.parent.parent / "experiments/toucan2020.json"
        with tempfile.TemporaryDirectory() as directory:
            # Kill inside prepare: finally cannot run, just as on CI cancellation.
            code = """
import importlib.util, os, signal, sys
spec = importlib.util.spec_from_file_location('experiment', sys.argv[1])
experiment = importlib.util.module_from_spec(spec)
spec.loader.exec_module(experiment)
experiment.prepare = lambda *args: os.kill(os.getpid(), signal.SIGKILL)
sys.argv = ['experiment', '--plan', sys.argv[2], '--work', sys.argv[3],
            '--baseline', sys.executable, '--candidate', sys.executable]
experiment.main()
"""
            result = subprocess.run([sys.executable, "-c", code, str(evaluator), str(plan), directory],
                                    capture_output=True, timeout=20)
            self.assertEqual(result.returncode, -signal.SIGKILL, result.stderr.decode())
            checkpoint = Path(directory) / "evidence/metadata.json"
            self.assertTrue(checkpoint.exists(), "cancellation erased the only identity checkpoint")
            meta = json.loads(checkpoint.read_text())
            self.assertEqual(meta["status"], "running")
            self.assertNotIn("runs", meta)
            self.assertEqual(meta["evaluator"]["sha256"], hashlib.sha256(evaluator.read_bytes()).hexdigest())
            self.assertEqual(meta["plan"]["sha256"], hashlib.sha256(plan.read_bytes()).hexdigest())
            binary_hash = hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest()
            for side in ("baseline", "candidate"):
                self.assertEqual(meta["binaries"][side]["sha256"], binary_hash)


if __name__ == "__main__":
    unittest.main()
