# /// script
# requires-python = ">=3.12,<3.14"
# dependencies = ["numpy==2.2.6", "openai-whisper==20250625", "torch==2.8.0"]
# ///
"""Run with: uv run --script scripts/test_transcription_shield.py"""

import tempfile
import unittest
import wave
from pathlib import Path

import numpy as np
from transcription_shield import (
    SAMPLE_RATE,
    audio_metrics,
    noise_control,
    validate_audio,
    word_error_rate,
    write_report,
    write_wav,
)


class TranscriptionShieldTests(unittest.TestCase):
    def test_word_errors_include_deletions_substitutions_and_insertions(self):
        self.assertEqual(word_error_rate("Ship on Friday", "Ship on Monday"), 1 / 3)
        self.assertEqual(word_error_rate("Ship on Friday", ""), 1)
        self.assertEqual(word_error_rate("Ship", "Ship it on Friday"), 3)
        self.assertEqual(word_error_rate("Let's ship!", "LET’S SHIP."), 0)
        self.assertEqual(
            word_error_rate(
                "The budget is twelve thousand dollars", "The budget is $12,000."
            ),
            0,
        )
        with self.assertRaises(ValueError):
            word_error_rate("", "Ship")

    def test_rejects_silence_nonfinite_stereo_clipping_and_long_clips(self):
        for audio in [
            np.zeros(SAMPLE_RATE),
            np.full(SAMPLE_RATE, np.nan),
            np.full(SAMPLE_RATE, np.inf),
            np.ones((SAMPLE_RATE, 2)),
            np.full(SAMPLE_RATE, 1.1),
            np.ones(30 * SAMPLE_RATE),
            np.ones(10),
        ]:
            with self.subTest(shape=audio.shape), self.assertRaises(ValueError):
                validate_audio(audio)
        validate_audio(np.full(SAMPLE_RATE, 0.1))

    def test_noise_control_preserves_perturbation_distribution_without_clipping(self):
        clean = np.full(1000, 0.25, dtype=np.float32)
        shield = clean + np.linspace(-0.01, 0.01, 1000, dtype=np.float32)
        original = clean.copy()
        noise = noise_control(clean, shield, 7)
        np.testing.assert_array_equal(clean, original)
        np.testing.assert_array_equal(np.sort(noise - clean), np.sort(shield - clean))
        np.testing.assert_array_equal(noise, noise_control(clean, shield, 7))
        self.assertFalse(np.array_equal(noise, shield))
        self.assertAlmostEqual(
            audio_metrics(clean, noise)["snr_db"],
            audio_metrics(clean, shield)["snr_db"],
        )

    def test_noise_control_rejects_permutations_that_would_clip(self):
        clean = np.array([1.0, 0.0], dtype=np.float32)
        shield = np.array([1.0, 0.1], dtype=np.float32)
        with self.assertRaisesRegex(ValueError, "headroom"):
            noise_control(clean, shield, 3)

    def test_metrics_do_not_silently_align_or_trim_mismatched_audio(self):
        self.assertFalse(audio_metrics(np.ones(10), np.ones(11))["length_matches"])
        self.assertIsNone(audio_metrics(np.ones(10), np.ones(11))["snr_db"])
        self.assertAlmostEqual(
            audio_metrics(np.ones(10), np.full(10, 1.1))["snr_db"], 20
        )

    def test_wav_roundtrip_preserves_clean_pcm_and_prevents_positive_wraparound(self):
        samples = np.array([-1, -0.5, 0, 0.5, 32767 / 32768, 1], dtype=np.float32)
        original = samples.copy()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "audio.wav"
            write_wav(path, samples)
            with wave.open(str(path)) as recording:
                self.assertEqual(recording.getframerate(), SAMPLE_RATE)
                self.assertEqual(recording.getnchannels(), 1)
                actual = np.frombuffer(recording.readframes(6), dtype="<i2")
        np.testing.assert_array_equal(actual, [-32768, -16384, 0, 16384, 32767, 32767])
        np.testing.assert_array_equal(samples, original)

    def test_listening_report_escapes_transcripts_and_reference(self):
        report = {
            "reference": "<script>bad()</script>",
            "attack_model": "tiny.en",
            "optimization": {"seconds": 1},
            "duration_seconds": 1,
            "epsilon": 0.01,
            "variants": {
                "shield": {
                    "snr_db": 20,
                    "length_matches": True,
                    "file": "shield.wav",
                    "transcripts": {
                        "tiny.en": {"wer": 1, "text": "<script>bad()</script>"}
                    },
                }
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            write_report(Path(directory), report)
            page = (Path(directory) / "index.html").read_text()
        self.assertNotIn("<script>", page)
        self.assertIn("&lt;script&gt;", page)
        self.assertIn("100.0%", page)


if __name__ == "__main__":
    unittest.main()
