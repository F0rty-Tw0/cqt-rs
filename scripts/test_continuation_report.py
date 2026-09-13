import unittest
from continuation_report import controlled


def start(song,t):
    return dict(event='start',song=song,t=t,consumed=t+1,estimated_start=t-2)


def end(song,t):
    return dict(event='end',song=song,t=t,consumed=t+1,estimated_end=t-4)


class IndependentRecountTests(unittest.TestCase):
    def test_repeated_plays_overlaps_misses_and_false_starts_remain_visible(self):
        truth=[dict(play_id='first',song='a',start=0,end=10),
               dict(play_id='overlap',song='b',start=4,end=14),
               dict(play_id='repeat',song='a',start=20,end=30),
               dict(play_id='miss',song='c',start=40,end=50)]
        events=[start('a',2),start('b',6),end('a',8),start('a',9),end('b',16),
                end('a',18),start('a',22),end('a',34),start('d',40),end('d',46)]
        r=controlled(events,truth)
        self.assertEqual((r['detected'],r['total'],r['duplicate_starts'],len(r['false_starts'])),(3,4,1,1))
        self.assertEqual(r['misses'],['miss'])
        self.assertEqual(r['premature_endings'],1)
        self.assertEqual(r['plays'][2]['end_event']['t'],34)
        self.assertEqual(r['timing']['notification_start_delay']['n'],3)
        self.assertEqual(sum(p['starts'] for p in r['plays'])+len(r['false_starts']),5)

    def test_missing_end_is_not_silently_counted_as_complete(self):
        with self.assertRaises(AssertionError): controlled([start('a',2)],[])


if __name__=='__main__':unittest.main()
