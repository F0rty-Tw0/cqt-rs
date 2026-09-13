"""Regression gates against attributing results to the wrong code or corpus."""

import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from eval_provenance import file_identity, manifest, validate_comparison


class ProvenanceTests(unittest.TestCase):
    def test_external_binary_is_identified_by_content_not_checkout_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            for name in ("radio_eval.py", "eval_provenance.py"):
                (root / "scripts" / name).write_text("# evaluator\n")
            binary = root / "old-monitor"
            binary.write_bytes(b"an executable from an older checkout")
            audio = root / "stream.wav"
            audio.write_bytes(b"audio fixture")
            with patch("eval_provenance.checkout_identity", return_value=dict(commit="new", dirty=True)):
                result = manifest(root, binary, {"programme.audio": audio}, built_here=False)
            self.assertEqual(result["evaluator"], dict(commit="new", dirty=True))
            self.assertFalse(result["monitor"]["built_from_evaluator_checkout"])
            self.assertNotIn("commit", result["monitor"])
            self.assertEqual(result["monitor"]["sha256"], hashlib.sha256(binary.read_bytes()).hexdigest())

    def test_replaced_input_at_same_path_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stream.wav"
            path.write_bytes(b"original audio")
            before = {"provenance": {"schema_version": 1, "inputs": {"programme.audio": file_identity(path)}}}
            path.write_bytes(b"different audio")
            after = {"provenance": {"schema_version": 1, "inputs": {"programme.audio": file_identity(path)}}}
            with self.assertRaisesRegex(ValueError, "programme.audio"):
                validate_comparison(before, after)

    def test_moved_inputs_and_changed_detector_are_comparable(self):
        def summary(path, binary, threshold):
            return dict(threshold=threshold, provenance=dict(
                schema_version=1, monitor=dict(sha256=binary),
                inputs={"watch:song": dict(path=path, sha256="same-content")}))
        validate_comparison(summary("/before/song.wav", "old", 70),
                            summary("/after/song.wav", "new", 75))

    def test_missing_or_added_inputs_are_rejected(self):
        before = dict(provenance=dict(schema_version=1, inputs={"watch:a": dict(sha256="a")}))
        after = dict(provenance=dict(schema_version=1, inputs={"watch:a": dict(sha256="a"),
                                                            "watch:b": dict(sha256="b")}))
        with self.assertRaisesRegex(ValueError, "watch:b"):
            validate_comparison(before, after)

    def test_legacy_opt_in_does_not_bypass_known_input_mismatch(self):
        with self.assertRaisesRegex(ValueError, "lacks input provenance"):
            validate_comparison({}, {})
        validate_comparison({}, {}, allow_unverified=True)
        a = dict(provenance=dict(schema_version=1, inputs={"truth": dict(sha256="a")}))
        b = dict(provenance=dict(schema_version=1, inputs={"truth": dict(sha256="b")}))
        with self.assertRaisesRegex(ValueError, "inputs differ"):
            validate_comparison(a, b, allow_unverified=True)


if __name__ == "__main__":
    unittest.main()
