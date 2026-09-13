"""Exercise the evaluator with replayed events, without a Rust build or audio download."""

import contextlib
import io
import json
import subprocess
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

import radio_eval


class EvaluationTests(unittest.TestCase):
    def test_cargo_reported_executable_handles_a_custom_target_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "custom-target" / "monitor"
            binary.parent.mkdir()
            binary.write_bytes(b"binary")
            output = json.dumps(dict(reason="compiler-artifact", target=dict(name="monitor"),
                                     executable=str(binary))) + "\n"
            with patch("radio_eval.subprocess.run", return_value=subprocess.CompletedProcess([], 0, output)):
                self.assertEqual(radio_eval.resolve_monitor(None), binary)

    def test_replayed_evaluation_saves_external_binary_and_corpus_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            binary = work / "old-monitor"
            binary.write_bytes(b"old binary")
            watch = work / "watch.wav"
            watch.write_bytes(b"reference fixture")
            for name in ("stream_null", "stream_eval"):
                (work / f"{name}.wav").write_bytes(name.encode())
            play = dict(source="song", watched=True, treatment="clean", start=1.0, end=5.0,
                        semitones=0, tempo=1.0, excerpt_start=0.0)
            (work / "stream_null.json").write_text(json.dumps(dict(segments=[])))
            (work / "stream_eval.json").write_text(json.dumps(dict(segments=[play])))
            stream = dict(event="stream", fingerprint_delay_seconds=0.5, window_seconds=5)
            done = dict(event="done", audio_seconds=6.0, realtime_fraction=0.01)
            report = dict(event="report", song="song", consumed=2.0, evidence=100,
                          confidence=80, verify_q=0.8)
            start = dict(event="start", song="song", consumed=2.0, t=1.5,
                         confidence=80, shift=0, tempo=1.0, position=0.5, verify_q=0.8)
            null_events = [stream, dict(report, evidence=1, confidence=2, verify_q=0), done]
            eval_events = [stream, report, start, done]
            args = Namespace(half=40, threshold=70, window=5, arg=[], programme="stream_eval",
                             monitor=str(binary), label=None, negatives=False, out="summary", no_plots=True)
            with (patch.object(radio_eval, "WORK", work),
                  patch.object(radio_eval, "WATCH", {"song": watch}),
                  patch.object(radio_eval, "run_monitor", side_effect=[null_events, eval_events]),
                  contextlib.redirect_stdout(io.StringIO()) as printed):
                result = radio_eval.evaluate(args)
            self.assertEqual(result["eval"]["detected"], 1)
            self.assertEqual(result["eval"]["false_starts"], 0)
            self.assertEqual(result["plays"][0]["position_error"], 0)
            self.assertEqual(result["monitor"], str(binary))
            self.assertTrue(result["label"].startswith("binary-"))
            self.assertFalse(result["provenance"]["monitor"]["built_from_evaluator_checkout"])
            self.assertEqual(len(result["provenance"]["inputs"]), 5)
            self.assertEqual(json.loads((work / "summary.json").read_text()), result)
            self.assertIn("not CPU utilization", printed.getvalue())


if __name__ == "__main__":
    unittest.main()
