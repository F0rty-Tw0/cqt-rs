import unittest
import numpy as np
from survival_diagnostic import verify,triplets,hash_survival,longest


class SurvivalOracleTests(unittest.TestCase):
    def test_known_mapping_and_unrelated_peaks(self):
        reference=np.array([[100,10],[120,20],[140,30]])
        query=np.array([[0,14],[10,24],[20,34]])
        good=verify(reference,query,2,100,4)
        self.assertEqual((good['query_matched'],good['reference_matched']),(3,3))
        mixed=np.array([[0,14],[5,60],[10,24],[15,70],[20,34]])
        dirty=verify(reference,mixed,2,100,4)
        self.assertEqual(dirty['reference_matched'],3)
        self.assertEqual(dirty['verify_q'],.6)
        removed=verify(reference,query[[0,2]],2,100,4)
        self.assertEqual(removed['reference_matched'],2)
        self.assertEqual(removed['verify_r'],2/3)
        wrong=verify(reference,query,2,100,20)
        self.assertEqual(wrong['query_matched'],0)

    def test_time_tolerance_is_directional_and_empty_is_zero(self):
        r=verify(np.array([[100,10]]),np.array([[4,10]]),2,90,0)
        self.assertEqual((r['query_matched'],r['reference_matched']),(1,1))
        self.assertEqual(verify(np.array([[10,10]]),np.empty((0,2)),1,0,0)['verify_q'],0)

    def test_exact_triplet_identity_and_index_cap(self):
        h=np.array(list(triplets([(100,10),(120,20),(140,30),(160,40)])))
        play=dict(start=0,end=2,source_start=0,tempo=1)
        got=hash_survival(h,h,play,0)
        self.assertEqual(got['supported_raw'],len(h))
        crowded=hash_survival(np.repeat(h,9,axis=0),h,play,0)
        self.assertEqual(crowded['supported_after_cap'],0)
        self.assertEqual(crowded['supported_raw'],len(h))

    def test_streak_requires_adjacent_windows(self):
        rows=[dict(begin_frame=0,end_frame=2,ok=True),dict(begin_frame=2,end_frame=4,ok=True),
              dict(begin_frame=6,end_frame=8,ok=True),dict(begin_frame=8,end_frame=10,ok=False)]
        self.assertEqual(longest(rows,'ok'),2)


class IndependentBruteForceTests(unittest.TestCase):
    def test_kdtree_counts_match_brute_force_at_boundaries(self):
        rng=np.random.default_rng(1502026)
        for _ in range(40):
            reference=np.unique(rng.integers([0,0],[200,60],size=(50,2)),axis=0)
            query=np.unique(rng.integers([0,0],[160,60],size=(40,2)),axis=0)
            tempo=float(rng.choice([.88,1,1.12]));offset=float(rng.choice([0,4,10.25]));shift=4
            lo=tempo*query[0,0]+offset-4;hi=tempo*query[-1,0]+offset+4
            ref=[r for r in reference if lo<=r[0]<=hi]
            qm=sum(any(abs(r[0]-(tempo*q[0]+offset))<=4 and abs(r[1]-(q[1]-shift))<=1 for r in ref) for q in query)
            rm=sum(any(abs(q[0]-(r[0]-offset)/tempo)<=4 and abs(q[1]-(r[1]+shift))<=1 for q in query) for r in ref)
            actual=verify(reference,query,tempo,offset,shift)
            self.assertEqual((actual['query_matched'],actual['reference_matched']),(qm,rm))


if __name__=="__main__":unittest.main()
