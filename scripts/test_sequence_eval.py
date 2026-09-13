"""Boundary attribution must retain misses, repeats, false and premature ends."""
import unittest
from sequence_eval import programme_score


class SequenceScoringTests(unittest.TestCase):
    def test_every_start_is_attributed_and_overlap_is_allowed(self):
        truth = [dict(play_id='a1', song='a', start=6, end=16),
                 dict(play_id='b1', song='b', start=14, end=24),
                 dict(play_id='a2', song='a', start=30, end=40)]
        events = [dict(event='start', song='a', t=10, consumed=12, estimated_start=6),
                  dict(event='end', song='a', t=13, consumed=15, estimated_end=12),
                  dict(event='start', song='a', t=14, consumed=16, estimated_start=12),
                  dict(event='start', song='b', t=16, consumed=18, estimated_start=14),
                  dict(event='start', song='x', t=18, consumed=20),
                  dict(event='end', song='a', t=20, consumed=22, estimated_end=16),
                  dict(event='end', song='b', t=28, consumed=30, estimated_end=24)]
        result = programme_score(events, truth)
        self.assertEqual(result['detected'], 2)
        self.assertEqual(result['duplicate_starts'], 1)
        self.assertEqual(len(result['false_starts']), 1)
        self.assertEqual(result['premature_endings'], 1)
        self.assertEqual(result['plays'][0]['notification_start_delay'], 6)
        self.assertEqual(result['plays'][0]['estimated_end_error'], -4)
        self.assertIsNone(result['plays'][2]['notification_start_delay'])

    def test_sequence_without_confirmed_start_is_a_miss(self):
        result = programme_score([dict(event='observation', song='a', t=8, confidence=99)],
                                 [dict(play_id='a', song='a', start=6, end=16)])
        self.assertEqual(result['detected'], 0)
        self.assertEqual(result['false_starts'], [])


if __name__ == '__main__':
    unittest.main()
