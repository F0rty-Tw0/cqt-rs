import unittest
from chunk_query_eval import score


class ScoringTest(unittest.TestCase):
    refs = {'a-0': dict(song='a', start_seconds=0),
            'a-1': dict(song='a', start_seconds=10),
            'b-0': dict(song='b', start_seconds=30)}

    @staticmethod
    def start(chunk, evidence, consumed=4):
        return dict(event='start', song=chunk, evidence=evidence, consumed=consumed, position=2)

    def test_competing_chunks_do_not_get_summed(self):
        events = [self.start('a-0', 60), self.start('a-1', 60), self.start('b-0', 100)]
        row = score(events, self.refs, 'a')
        self.assertEqual(row['outcome'], 'wrong-match')
        self.assertEqual(row['predicted'], 'b')
        self.assertEqual(row['best']['original_position'], 32)
        self.assertEqual(row['accepted_parent_songs'], ['a', 'b'])
        self.assertEqual(row['repeated_parent_starts'], 1)

    def test_query_label_cannot_change_prediction(self):
        events = [self.start('a-1', 100), self.start('b-0', 99)]
        a, b = (score(events, self.refs, expected) for expected in ['a', 'b'])
        self.assertEqual(a['predicted'], b['predicted'])
        self.assertEqual(a['best']['original_position'], 12)
        self.assertEqual(a['outcome'], 'correct')
        self.assertEqual(b['outcome'], 'wrong-match')

    def test_reports_are_not_accepted_matches(self):
        row = score([dict(event='report', song='a-0', evidence=9000)], self.refs, 'a')
        self.assertEqual(row['outcome'], 'no-match')

    def test_deterministic_ties_and_unknown_chunk_rejected(self):
        events = [self.start('b-0', 100, 3), self.start('a-1', 100, 2)]
        self.assertEqual(score(events, self.refs, 'b')['predicted'], 'a')
        with self.assertRaises(KeyError):
            score([self.start('missing', 100)], self.refs, 'a')


if __name__ == '__main__':
    unittest.main()
