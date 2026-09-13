import tempfile
import unittest
from pathlib import Path
from continuation_eval import validate_resume
import real_mix_eval as e


class ContinuationResumeTests(unittest.TestCase):
    def test_changed_configuration_and_output_cannot_reuse_result(self):
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)/'out';err=Path(folder)/'err'
            out.write_text('native output\n');err.write_text('')
            signature={'binary':'a', 'input':'b', 'options':['--window','10']}
            row=dict(signature=signature,status='completed',stdout=e.identity(out),stderr=e.identity(err))
            validate_resume(row,signature)
            with self.assertRaises(ValueError):
                validate_resume(row,dict(signature,options=['--window','5']))
            err.write_text('changed')
            with self.assertRaises(ValueError):
                validate_resume(row,signature)

    def test_json_round_trip_keeps_reference_identity_and_rejects_real_changes(self):
        import json
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)/'out';out.write_text('native output')
            signature={'references':[('a','sha-a')], 'binary':'sha-b', 'evaluator':'sha-c'}
            row=dict(signature=json.loads(json.dumps(signature)),status='completed',
                     stdout=e.identity(out),stderr=e.identity(out))
            validate_resume(row,signature)
            with self.assertRaises(ValueError):
                validate_resume(row,dict(signature,references=[('a','different')]))
            with self.assertRaises(ValueError):
                validate_resume(row,dict(signature,evaluator='different'))

    def test_failed_or_incomplete_run_is_not_a_completed_cache(self):
        with self.assertRaises(ValueError):
            validate_resume({'status':'running','signature':{}},{})


if __name__ == '__main__':
    unittest.main()
