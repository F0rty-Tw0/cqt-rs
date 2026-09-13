"""Scoring must preserve misses, duplicate starts and out-of-window alarms."""
import unittest
from recognition_lab import score


class KnownTruthTests(unittest.TestCase):
    def test_spurious_first_start_does_not_hide_a_later_correct_detection(self):
        truth=[dict(song='a',start=5.,end=15.,tempo=1.),dict(song='b',start=30.,end=40.,tempo=1.)]
        def start(song,at):
            return dict(event='start',song=song,t=at-0.4,consumed=at,position=at-5.4)
        events=[start('a',1.),start('a',7.),start('a',9.),start('b',45.)]
        result=score(events,truth,['a','b'])
        self.assertEqual(result['detected'],1)
        self.assertEqual(result['misses'],['b'])
        self.assertEqual(result['first_starts']['a']['consumed'],7.)
        self.assertEqual(len(result['false_starts']),2)
        self.assertEqual(result['duplicates'],1)
        self.assertEqual(result['start_delays'],[2.])

    def test_every_start_on_unwatched_music_is_false(self):
        events=[dict(event='start',song='a',t=1.,consumed=1.4,position=2.)]
        result=score(events,[],['a'])
        self.assertEqual(result['detected'],0)
        self.assertEqual(result['total'],0)
        self.assertEqual(result['false_starts'],events)


if __name__=='__main__':unittest.main()
