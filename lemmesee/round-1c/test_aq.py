"""Known answers on Chinook, from the snapshot round 1's tests keep.

    python -I test_aq.py
"""

import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))

import gold  # noqa: E402
import rank  # noqa: E402
from aq import generate, identity, load  # noqa: E402

SNAPSHOT = HERE.parent / "round-1" / "harness" / "rules" / "tests" / "data" / "chinook-postgres-fresh.json"
GOLD = HERE.parent / "round-1" / "gold" / "questions.json"


def chinook(reads_n=True):
    return load(json.loads(SNAPSHOT.read_text(encoding="utf-8")), reads_n)


def table(schema, name):
    return next(t for t in schema.tables if t.name == name)


class Chinook(unittest.TestCase):
    def test_invoices_are_the_subject_and_their_lines_hang_on_them(self):
        for reads_n in (False, True):
            s = chinook(reads_n)
            self.assertEqual(table(s, "invoice").subject, 0)
            self.assertEqual(table(s, "invoice_line").detail_of, table(s, "invoice").t)
            self.assertIsNone(table(s, "track").subject)

    def test_a_lines_revenue_is_its_price_times_its_quantity(self):
        s = chinook()
        line = table(s, "invoice_line")
        kinds = {q.measure.kind for q in generate(s) if q.base == line.t}
        self.assertIn("product", kinds)

    def test_no_catalogue_price_is_multiplied_by_a_duration(self):
        s = chinook()
        track = table(s, "track")
        for q in generate(s):
            if q.base == track.t and q.measure.kind == "product":
                self.fail(q.title(s))

    def test_every_question_compiles_for_both_engines(self):
        s = chinook()
        for q in generate(s):
            for engine in ("postgres", "mysql"):
                sql = q.sql(s, engine)
                self.assertTrue(sql.startswith("SELECT "), sql)
                self.assertNotIn("None", sql)

    def test_a_line_takes_its_invoices_time(self):
        s = chinook()
        line, invoice = table(s, "invoice_line"), table(s, "invoice")
        times = {(q.time.t, q.time.c) for q in generate(s) if q.base == line.t and q.time}
        self.assertEqual({t for t, _ in times}, {invoice.t})

    def test_gold_questions_are_said(self):
        s = chinook()
        ids = {identity(s, q) for q in generate(s)}
        spec = json.loads(GOLD.read_text(encoding="utf-8"))["fixtures"]["chinook"]
        said = [gold.identity(s, q)[0] in ids for q in spec["questions"]]
        # all but revenue by genre month by month: 25 genres are too many to split by
        self.assertEqual(sum(said), len(said) - 1, said)

    def test_tracks_are_named_and_invoices_are_not(self):
        s = chinook()
        line, track, invoice = table(s, "invoice_line"), table(s, "track"), table(s, "invoice")
        kinds = {(q.by.t, q.by.kind) for q in generate(s) if q.base == line.t and q.by and q.by.c == -1}
        self.assertIn((track.t, "label"), kinds)
        self.assertIn((invoice.t, "key"), kinds)
        self.assertNotIn((invoice.t, "label"), kinds)

    def test_the_five_never_repeats_a_time_or_a_breakdown(self):
        s = chinook()
        qs = [q for q in generate(s) if rank.veto(s, q) is None]
        scores = [rank.prior(rank.features(s, q)) for q in qs]
        five = [qs[i] for i in rank.choose(s, qs, scores)]
        self.assertEqual(len(five), 5)
        times = [(q.time.t, q.time.c) for q in five if q.time]
        bys = [identity(s, q)[3] for q in five if q.by]
        self.assertEqual(len(times), len(set(times)))
        self.assertEqual(len(bys), len(set(bys)))


if __name__ == "__main__":
    unittest.main()
