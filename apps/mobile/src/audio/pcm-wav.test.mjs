import assert from "node:assert/strict";
import test from "node:test";

import { pcmAmplitude, recoverableWav, wavHeader } from "./pcm-wav.ts";

test("writes a little-endian PCM WAV header", () => {
  const header = wavHeader(32_000, 16_000, 1);
  const view = new DataView(header.buffer);

  assert.equal(new TextDecoder().decode(header.slice(0, 4)), "RIFF");
  assert.equal(view.getUint32(4, true), 32_036);
  assert.equal(new TextDecoder().decode(header.slice(8, 12)), "WAVE");
  assert.equal(view.getUint16(20, true), 1);
  assert.equal(view.getUint16(22, true), 1);
  assert.equal(view.getUint32(24, true), 16_000);
  assert.equal(view.getUint32(28, true), 32_000);
  assert.equal(view.getUint16(34, true), 16);
  assert.equal(view.getUint32(40, true), 32_000);
});

test("normalizes int16 PCM amplitude", () => {
  const samples = new Int16Array([0, 0, 0, 0, 16_384, 0, 0, 0]);
  assert.equal(pcmAmplitude(samples.buffer), 0.5);
  assert.equal(pcmAmplitude(new Int16Array([-32_768]).buffer), 1);
});

test("recovers PCM format and aligns bytes from an interrupted file", () => {
  const header = wavHeader(0, 16_000, 1);

  assert.deepEqual(recoverableWav(header, 44 + 32_001), {
    channels: 1,
    dataBytes: 32_000,
    sampleRate: 16_000,
  });
});

test("rejects empty, corrupt, and unsupported WAV files", () => {
  assert.equal(recoverableWav(wavHeader(0, 16_000, 1), 44), null);
  assert.equal(recoverableWav(wavHeader(0, 16_000, 1), Number.NaN), null);

  const corrupt = wavHeader(32_000, 16_000, 1);
  corrupt[0] = 0;
  assert.equal(recoverableWav(corrupt, 44 + 32_000), null);

  const inconsistent = wavHeader(32_000, 16_000, 1);
  new DataView(inconsistent.buffer).setUint32(28, 0, true);
  assert.equal(recoverableWav(inconsistent, 44 + 32_000), null);

  const unsupported = wavHeader(32_000, 16_000, 3);
  assert.equal(recoverableWav(unsupported, 44 + 32_000), null);
});
