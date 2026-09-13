import random
import unittest

from song_index_prepare import choose_start, intervals_for, verify_title
from song_index_eval import outcome, validate_events


class SongIndexTest(unittest.TestCase):
    def test_mislabeled_download_rejected(self):
        expected = dict(title='Roots and Shoots')
        with self.assertRaisesRegex(ValueError, 'source title mismatch'):
            verify_title(expected, dict(title='Exodus (original mix)', artist='Marc Burt'))
        verify_title(expected, dict(title='Roots and Shoots', artist='Dave Kent'))

    def test_interval_union_does_not_overweight_overlapping_anchors(self):
        anchors = [dict(mix_start=0,mix_end=20), dict(mix_start=5,mix_end=25),
                   dict(mix_start=30,mix_end=40), dict(mix_start=50,mix_end=55)]
        self.assertEqual(intervals_for(anchors), [[0,15*44100],[30*44100,30*44100]])
        intervals = [[2,4],[10,11]]
        class Draw:
            def __init__(self, n): self.n = n
            def randrange(self, size):
                assert size == 5
                return self.n
        self.assertEqual([choose_start(intervals,Draw(i)) for i in range(5)], [2,3,4,10,11])
        a,b = random.Random('frozen'), random.Random('frozen')
        self.assertEqual(choose_start(intervals,a), choose_start(intervals,b))

    def test_negative_acceptance_cannot_be_scored_as_correct(self):
        refs = {'a':dict(song='a',start_seconds=0)}
        self.assertEqual(outcome([],refs,None)['outcome'], 'correct-rejection')
        start = dict(event='start',song='a',evidence=100,consumed=5,position=3)
        result = outcome([start],refs,None)
        self.assertEqual(result['outcome'],'false-accept')
        self.assertEqual(len(result['wrong_parent_starts']),1)

    def test_partial_query_and_incomplete_index_are_rejected(self):
        refs = {'a':dict(seconds=10)}
        query = dict(id='q')
        events = [dict(event='index',song='a',seconds=10),dict(event='done',audio_seconds=10)]
        validate_events(events,refs,query)
        with self.assertRaises(ValueError):
            validate_events(events[1:],refs,query)
        events[-1]['audio_seconds'] = 9.5
        with self.assertRaises(ValueError):
            validate_events(events,refs,query)


if __name__ == '__main__':
    unittest.main()
