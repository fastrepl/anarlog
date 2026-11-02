import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("score.py")
SPEC = importlib.util.spec_from_file_location("vi_en_score", SCRIPT)
score = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(score)


class ScoreTest(unittest.TestCase):
    def test_unicode_normalization_handles_diacritics_and_punctuation(self):
        self.assertEqual(score.normalize("Chưa duyệt, STAGING!"), "chưa duyệt staging")

    def test_edit_distance_counts_replacements_insertions_and_deletions(self):
        self.assertEqual(
            score.edit_distance(["chưa", "duyệt"], ["đã", "duyệt", "rồi"]), 2
        )

    def test_critical_terms_use_phrase_boundaries(self):
        self.assertTrue(score.phrase_present("Redis", "Move to Redis by Friday"))
        self.assertFalse(score.phrase_present("Redis", "Use Redisearch instead"))

    def test_scoring_keeps_dev_rows_out_of_test_report(self):
        rows = [
            {
                "meeting_id": "m1",
                "split": split,
                "speaker_group": "group-a",
                "device": "iphone17",
                "candidate": "whisper",
                "reference": "Chưa duyệt deploy.",
                "hypothesis": hypothesis,
                "critical_terms": ["chưa duyệt"],
                "audio_seconds": 10,
                "inference_seconds": inference,
                "peak_rss_mb": 100,
            }
            for split, hypothesis, inference in (
                ("dev", "Sai deploy.", 20),
                ("test", "Chưa duyệt deploy.", 5),
            )
        ]
        report = score.score_records(rows, "test")["whisper [iphone17]"]
        self.assertEqual(report["meetings"], 1)
        self.assertEqual(report["wer"], 0)
        self.assertEqual(report["critical_accuracy"], 1)
        self.assertEqual(report["rtf"], 0.5)

    def test_candidates_must_have_identical_meeting_references(self):
        base = {
            "meeting_id": "m1",
            "split": "test",
            "speaker_group": "group-a",
            "device": "iphone17",
            "reference": "Chưa duyệt.",
            "hypothesis": "Chưa duyệt.",
            "critical_terms": [],
            "audio_seconds": 10,
        }
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "results.jsonl"
            manifest.write_text(
                "\n".join(
                    json.dumps({**base, "candidate": candidate, "reference": reference})
                    for candidate, reference in (
                        ("a", "Chưa duyệt."),
                        ("b", "Đã duyệt."),
                    )
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "must share split"):
                score.load_records(manifest)

    def test_candidates_must_use_the_same_meetings_on_each_device(self):
        base = {
            "split": "test",
            "speaker_group": "group-a",
            "device": "iphone17",
            "reference": "Chưa duyệt.",
            "hypothesis": "Chưa duyệt.",
        }
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "results.jsonl"
            manifest.write_text(
                "\n".join(
                    json.dumps({**base, "meeting_id": meeting, "candidate": candidate})
                    for meeting, candidate in (("m1", "a"), ("m1", "b"), ("m2", "a"))
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "must use the same meetings"):
                score.load_records(manifest)

    def test_quality_and_resource_scores_are_reported_per_device(self):
        rows = []
        for device, inference in (("iphone17", 8), ("macbook16", 4)):
            rows.append(
                {
                    "meeting_id": "m1",
                    "split": "test",
                    "speaker_group": "group-a",
                    "device": device,
                    "candidate": "whisper",
                    "reference": "Chưa duyệt deploy.",
                    "hypothesis": "Chưa duyệt deploy.",
                    "critical_terms": [],
                    "audio_seconds": 10,
                    "inference_seconds": inference,
                    "peak_rss_mb": 100,
                }
            )
        reports = score.score_records(rows, "test")
        self.assertEqual(reports["whisper [iphone17]"]["rtf"], 0.8)
        self.assertEqual(reports["whisper [macbook16]"]["rtf"], 0.4)


if __name__ == "__main__":
    unittest.main()
