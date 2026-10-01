"""The README must not turn a failed stage into a successful figure."""

import json
import pathlib
import tempfile
import unittest

import cards


class CardsTest(unittest.TestCase):
    def load_round(self, round_):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            (path / "metrics.json").write_text(json.dumps(round_))
            return cards.load(path, "metrics")

    def test_failed_runs_are_refused(self):
        with self.assertRaises(ValueError):
            self.load_round({"runs": [], "failed": ["tinystore repeat 1"]})

    def test_failed_stages_are_refused(self):
        with self.assertRaises(ValueError):
            self.load_round({"runs": [{"contender": "tinystore", "stages": [
                {"name": "read-wide", "errors": 1},
            ]}]})

    def test_ingest_includes_settling(self):
        round_ = {"runs": [{"contender": "tinystore", "stages": [
            {"name": "ingest", "ops": 100, "seconds": 2},
            {"name": "settle", "ops": 1, "seconds": 3},
        ]}]}
        self.assertEqual(cards.settled_ingest(round_, "tinystore"), 20)

    def test_memory_keeps_the_service_and_client(self):
        round_ = {"runs": [{"contender": "tinystore", "peak_rss_bytes": 1 << 20,
                            "service_pss_bytes": 3 << 20, "service_peak_rss_bytes": 2 << 20}]}
        self.assertEqual(cards.peak(round_, "tinystore"), 4)


if __name__ == "__main__":
    unittest.main()
