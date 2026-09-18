# /// script
# requires-python = ">=3.12,<3.14"
# dependencies = ["numpy==2.2.6", "openai-whisper==20250625", "torch==2.8.0"]
# ///
"""Offline sword/shield experiment. Requires ffmpeg; downloads local Whisper models.

uv run --script scripts/transcription_shield.py speech.wav \
    --reference "The words spoken in the recording." --output /tmp/shield-run

Fits a perturbation to one complete utterance, not a real-time or universal shield.
No audio is uploaded. Outputs WAVs, transcripts, metrics, and a listening page.
"""

import argparse
import hashlib
import html
import importlib.metadata
import json
import math
import shutil
import subprocess
import time
import wave
from pathlib import Path

import numpy as np

SAMPLE_RATE = 16000


def words(text):
    from whisper.normalizers import EnglishTextNormalizer

    return EnglishTextNormalizer()(text.replace("’", "'")).split()


def word_error_rate(reference, hypothesis):
    expected, actual = words(reference), words(hypothesis)
    if not expected:
        raise ValueError("The reference must contain at least one word")
    previous = list(range(len(actual) + 1))
    for i, left in enumerate(expected, 1):
        current = [i]
        for j, right in enumerate(actual, 1):
            current.append(
                min(
                    previous[j] + 1,
                    current[j - 1] + 1,
                    previous[j - 1] + (left != right),
                )
            )
        previous = current
    return previous[-1] / len(expected)


def validate_audio(audio):
    if audio.ndim != 1 or not np.isfinite(audio).all():
        raise ValueError("Audio must be a finite mono waveform")
    if not SAMPLE_RATE // 2 <= len(audio) <= 29 * SAMPLE_RATE:
        raise ValueError(
            "Use a clip between 0.5 and 29 seconds; audio is never truncated"
        )
    if np.max(np.abs(audio)) > 1:
        raise ValueError("Audio samples must be between -1 and 1")
    if np.sqrt(np.mean(audio.astype(np.float64) ** 2)) < 1e-5:
        raise ValueError("Audio is silent or too quiet to evaluate")


def write_wav(path, audio):
    pcm = np.rint(np.clip(audio, -1, 1 - 1 / 32768) * 32768).astype("<i2")
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(SAMPLE_RATE)
        output.writeframes(pcm.tobytes())


def audio_metrics(clean, processed):
    if clean.shape != processed.shape:
        return {"snr_db": None, "peak_delta": None, "length_matches": False}
    delta = processed.astype(np.float64) - clean.astype(np.float64)
    power = float(np.mean(delta**2))
    return {
        "snr_db": 10 * math.log10(float(np.mean(clean.astype(np.float64) ** 2)) / power)
        if power
        else None,
        "peak_delta": float(np.max(np.abs(delta))),
        "length_matches": True,
    }


def noise_control(clean, shield, seed):
    # Shuffling preserves the perturbation's energy and amplitude distribution.
    delta = np.random.default_rng(seed).permutation(shield - clean)
    control = clean + delta
    if np.any(np.abs(control) > 1):
        raise ValueError("Audio needs more headroom for an unclipped noise control")
    return control


def opus_roundtrip(source, target):
    encoded = target.with_suffix(".opus")
    for command in [
        [
            "-i",
            str(source),
            "-c:a",
            "libopus",
            "-b:a",
            "24k",
            "-application",
            "voip",
            "-frame_duration",
            "20",
            str(encoded),
        ],
        ["-i", str(encoded), "-ar", str(SAMPLE_RATE), "-ac", "1", str(target)],
    ]:
        subprocess.run(
            ["ffmpeg", "-nostdin", "-v", "error", "-n", *command],
            check=True,
            capture_output=True,
        )


def transcribe(model, audio):
    result = model.transcribe(
        audio,
        language="en",
        task="transcribe",
        fp16=False,
        temperature=0.0,
        condition_on_previous_text=False,
        without_timestamps=True,
        verbose=None,
    )
    return result["text"].strip()


def optimize(model, clean, baseline, steps, epsilon, seed):
    import torch
    import whisper
    from whisper.tokenizer import get_tokenizer

    tokenizer = get_tokenizer(
        model.is_multilingual,
        num_languages=model.num_languages,
        language="en",
        task="transcribe",
    )
    prefix = list(tokenizer.sot_sequence_including_notimestamps)
    target = tokenizer.encode(" " + baseline.strip()) + [tokenizer.eot]
    if len(prefix) + len(target) > model.dims.n_text_ctx:
        raise ValueError("The clean transcript exceeds the model's text context")
    decoder_input = torch.tensor([prefix + target[:-1]], device=model.device)
    expected = torch.tensor(target, device=model.device)
    source = torch.from_numpy(clean.copy())
    rng = np.random.default_rng(seed)
    candidate = torch.clamp(
        source
        + torch.from_numpy(
            rng.uniform(-epsilon, epsilon, clean.shape).astype(np.float32)
        ),
        -1,
        1,
    )
    lower, upper = (source - epsilon).clamp(-1, 1), (source + epsilon).clamp(-1, 1)
    step_size = epsilon / 4
    losses = []
    best_loss, best = -math.inf, candidate.clone()
    for parameter in model.parameters():
        parameter.requires_grad_(False)
    started = time.perf_counter()
    for step in range(steps + 1):
        candidate = candidate.detach().requires_grad_(True)
        # STFT stays on CPU; the differentiable transfer also supports Metal models.
        mel = whisper.log_mel_spectrogram(
            whisper.pad_or_trim(candidate),
            n_mels=model.dims.n_mels,
        ).to(model.device)
        logits = model(mel.unsqueeze(0), decoder_input)[0, len(prefix) - 1 :]
        loss = torch.nn.functional.cross_entropy(logits, expected)
        value = float(loss.detach().cpu())
        if not math.isfinite(value):
            raise RuntimeError("Optimization produced a non-finite loss")
        if value > best_loss:
            best_loss, best = value, candidate.detach().clone()
        losses.append(value)
        if step % 10 == 0 or step == steps:
            print(f"  step {step}/{steps}: transcript loss {value:.4f}", flush=True)
        if step == steps:
            break
        (gradient,) = torch.autograd.grad(loss, candidate)
        if not torch.isfinite(gradient).all():
            raise RuntimeError("Optimization produced a non-finite gradient")
        candidate = torch.maximum(
            lower,
            torch.minimum(upper, candidate + step_size * gradient.sign()),
        )
    return best.numpy(), {
        "seconds": time.perf_counter() - started,
        "losses": losses,
        "best_loss": best_loss,
        "step_size": step_size,
    }


def write_report(output, report):
    (output / "report.json").write_text(
        json.dumps(report, indent=2, allow_nan=False) + "\n"
    )
    cards = []
    for name, variant in report["variants"].items():
        rows = "".join(
            f"<tr><td>{html.escape(model)}</td><td>{result['wer']:.1%}</td>"
            f"<td>{html.escape(result['text']) or '<em>(empty)</em>'}</td></tr>"
            for model, result in variant["transcripts"].items()
        )
        snr = variant["snr_db"]
        detail = f"SNR {snr:.1f} dB" if snr is not None else "Clean reference"
        if not variant["length_matches"]:
            detail = "Length differs; SNR not computed"
        cards.append(
            f"<section><h2>{html.escape(name.replace('_', ' '))}</h2>"
            f"<p>{detail}</p><audio controls preload='none' src='{variant['file']}'></audio>"
            "<table><thead><tr><th>Model</th><th>Word error rate</th><th>Transcript</th></tr>"
            f"</thead><tbody>{rows}</tbody></table></section>"
        )
    (output / "index.html").write_text(
        "<!doctype html><html lang='en'><meta charset='utf-8'>"
        "<meta name='viewport' content='width=device-width,initial-scale=1'>"
        "<title>Sword &amp; shield · Anarlog experiment</title><style>"
        "body{font:16px/1.6 system-ui;background:#f6f4ef;color:#20241f;max-width:1000px;"
        "margin:48px auto;padding:0 24px}h1{font-size:42px;letter-spacing:-1.5px}"
        "h2{text-transform:capitalize}section{background:white;border:1px solid #ddd;"
        "border-radius:14px;padding:24px;margin:20px 0}audio{width:100%}"
        "table{width:100%;border-collapse:collapse;margin-top:20px}"
        "td,th{text-align:left;padding:10px;border-bottom:1px solid #eee;vertical-align:top}"
        "td:first-child,td:nth-child(2){white-space:nowrap}th{font-size:13px;color:#555}"
        "@media(max-width:640px){td,th{padding:5px;font-size:13px}h1{font-size:32px}}"
        "</style><h1>Sword &amp; shield</h1><p>Offline transcription experiment · ANLG-393</p>"
        "<p><strong>Reference:</strong> " + html.escape(report["reference"]) + "</p>"
        "<p>The sword gets clean audio. The shield is fitted to this exact clip and "
        + html.escape(report["attack_model"])
        + ". The other model is a held-out test. "
        "The noise control shuffles the same perturbation. Opus uses 24 kbps voice encoding.</p>"
        f"<p>Optimization: {report['optimization']['seconds']:.1f}s for "
        f"{report['duration_seconds']:.1f}s of audio; ε={report['epsilon']}. "
        "This is not a live call, a universal shield, or evidence of human intelligibility. "
        "Listen to compare. Word error rate can exceed 100% when words are inserted.</p>"
        + "".join(cards)
        + "<p><a href='report.json'>Full measurements and settings</a></p></html>"
    )


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("audio", type=Path)
    parser.add_argument("--reference", required=True)
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="New directory; existing paths are refused",
    )
    parser.add_argument(
        "--attack-model", choices=["tiny.en", "base.en", "small.en"], default="tiny.en"
    )
    parser.add_argument(
        "--eval-model", choices=["tiny.en", "base.en", "small.en"], default="base.en"
    )
    parser.add_argument("--steps", type=int, default=40)
    parser.add_argument(
        "--epsilon",
        type=float,
        default=0.01,
        help="Maximum sample change, full scale=1",
    )
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--device", choices=["cpu", "mps", "cuda"], default="cpu")
    args = parser.parse_args()
    if not 1 <= args.steps <= 1000 or not 0 < args.epsilon <= 0.1 or args.threads < 1:
        parser.error("Use 1–1000 steps, 0 < epsilon <= 0.1, and at least one thread")
    if args.attack_model == args.eval_model:
        parser.error(
            "Evaluation model must differ from the model used for optimization"
        )
    if not words(args.reference):
        parser.error("Reference must contain words")
    if not args.audio.is_file() or not shutil.which("ffmpeg"):
        parser.error("An existing audio file and ffmpeg are required")
    if args.output.exists():
        parser.error("Output already exists; choose a new directory")

    import torch
    import whisper

    torch.set_num_threads(args.threads)
    torch.manual_seed(args.seed)
    clean = whisper.load_audio(str(args.audio))
    validate_audio(clean)
    args.output.mkdir(parents=True)
    write_wav(args.output / "clean.wav", clean)
    print(f"Loading {args.attack_model} on {args.device}", flush=True)
    model = whisper.load_model(args.attack_model, device=args.device).eval()
    baseline = transcribe(model, clean)
    if not words(baseline):
        raise ValueError(
            "Clean audio produced no words; cannot fit a meaningful shield"
        )
    print(f"Clean transcript: {baseline}", flush=True)
    shield, optimization = optimize(
        model,
        clean,
        baseline,
        args.steps,
        args.epsilon,
        args.seed,
    )
    write_wav(args.output / "shield.wav", shield)
    write_wav(
        args.output / "noise_control.wav", noise_control(clean, shield, args.seed)
    )
    variants = {}
    for name in ["clean", "shield", "noise_control"]:
        source = args.output / f"{name}.wav"
        opus_roundtrip(source, args.output / f"{name}_opus.wav")
        for variant_name in [name, f"{name}_opus"]:
            path = args.output / f"{variant_name}.wav"
            waveform = whisper.load_audio(str(path))
            variants[variant_name] = {
                "file": path.name,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "sample_count": len(waveform),
                **audio_metrics(clean, waveform),
                "transcripts": {},
            }
    report = {
        "reference": args.reference,
        "attack_model": args.attack_model,
        "eval_model": args.eval_model,
        "steps": args.steps,
        "epsilon": args.epsilon,
        "seed": args.seed,
        "device": args.device,
        "threads": args.threads,
        "sample_rate": SAMPLE_RATE,
        "wer_normalizer": "whisper.normalizers.EnglishTextNormalizer",
        "duration_seconds": len(clean) / SAMPLE_RATE,
        "input_sha256": hashlib.sha256(args.audio.read_bytes()).hexdigest(),
        "packages": {
            name: importlib.metadata.version(name)
            for name in ["torch", "numpy", "openai-whisper"]
        },
        "decoding": {
            "language": "en",
            "temperature": 0,
            "without_timestamps": True,
            "condition_on_previous_text": False,
        },
        "codec": {
            "name": "libopus",
            "bitrate": "24k",
            "application": "voip",
            "frame_ms": 20,
        },
        "optimization": optimization,
        "variants": variants,
        "limitations": [
            "Fitted to one entire utterance, not real time",
            "Only Whisper family evaluated",
            "Opus is not an actual meeting pipeline",
            "Human intelligibility and summary recovery not evaluated",
        ],
    }
    for model_name in [args.attack_model, args.eval_model]:
        if model_name != args.attack_model:
            del model
            model = whisper.load_model(model_name, device=args.device).eval()
        for name, variant in variants.items():
            text = transcribe(
                model, whisper.load_audio(str(args.output / variant["file"]))
            )
            wer = word_error_rate(args.reference, text)
            variant["transcripts"][model_name] = {"text": text, "wer": wer}
            print(f"{model_name:9} {name:19} WER={wer:.1%} {text}", flush=True)
        write_report(args.output, report)
    print(f"Listening page: {args.output.resolve() / 'index.html'}", flush=True)


if __name__ == "__main__":
    main()
