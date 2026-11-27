import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { test } from "node:test";

const files = (globalThis.wavWriterFiles = new Map());
registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "@/lib/ids")
      return {
        url: `data:text/javascript,${encodeURIComponent("export {randomUUID as id} from 'node:crypto';")}`,
        shortCircuit: true,
      };
    if (specifier === "@/audio/pcm-wav")
      return {
        url: new URL("./pcm-wav.ts", import.meta.url).href,
        shortCircuit: true,
      };
    if (specifier === "expo-file-system")
      return { url: "test:wav-files", shortCircuit: true };
    return next(specifier, context);
  },
  load(url, context, next) {
    if (url !== "test:wav-files") return next(url, context);
    return {
      format: "module",
      shortCircuit: true,
      source: `
      export const Paths = { document: "/test" };
      export const FileMode = { ReadWrite: "rw", ReadOnly: "r" };
      export class Directory { constructor(...parts) { this.path = parts.join("/"); } create() {} }
      export class File {
        exists = false;
        bytes = new Uint8Array(0);
        constructor(directory, filename) { this.name = filename; this.path = directory.path + "/" + filename; const prior = globalThis.wavWriterFiles.get(this.path); if (prior) return prior; globalThis.wavWriterFiles.set(this.path, this); }
        get size() { return this.bytes.length; }
        create() { if (this.exists) throw Error("File exists"); this.exists = true; }
        copy(target) { target.bytes = this.bytes.slice(); target.exists = true; }
        delete() { this.exists = false; }
        open() {
          const file = this;
          return { offset: 0, readBytes(length) { const bytes = file.bytes.slice(this.offset, this.offset + length); this.offset += bytes.length; return bytes; }, writeBytes(bytes) {
            if (file.failWrite) throw Error("Disk write failed");
            const result = new Uint8Array(Math.max(file.bytes.length, this.offset + bytes.length));
            result.set(file.bytes); result.set(bytes, this.offset); file.bytes = result; this.offset += bytes.length;
          }, close() {} };
        }
      }
    `,
    };
  },
});
const { SessionWavWriter } = await import("./session-wav-writer.ts");

test("elapsed recording time comes from captured PCM and matches the finalized WAV", () => {
  for (const [sampleRate, channels] of [
    [16000, 1],
    [48000, 2],
  ]) {
    const writer = new SessionWavWriter(`duration-${channels}`);
    assert.equal(writer.durationMs, 0);
    const bytesPerFrame = (sampleRate * channels * 2) / 10;
    for (let i = 0; i < 400; i++)
      writer.append(new ArrayBuffer(bytesPerFrame), sampleRate, channels);
    assert.equal(writer.durationMs, 40000);
    const file = writer.finalize();
    const header = new DataView(file.bytes.buffer);
    assert.equal(
      (header.getUint32(40, true) / header.getUint32(28, true)) * 1000,
      writer.durationMs,
    );
    assert.equal(file.size, 44 + bytesPerFrame * 400);
  }
});

test("failed audio writes do not advance the recording duration", () => {
  const writer = new SessionWavWriter("failed-write");
  writer.append(new ArrayBuffer(3200), 16000, 1);
  writer.file.failWrite = true;
  assert.throws(
    () => writer.append(new ArrayBuffer(3200), 16000, 1),
    /Disk write failed/,
  );
  assert.equal(writer.durationMs, 100);
  assert.equal(files.get("/test/sessions/failed-write/audio.wav").size, 3244);
});

test("resuming preserves prior audio and appends PCM across multiple stops without including the pause", () => {
  const first = new SessionWavWriter("resume");
  const original = new Uint8Array(3200).fill(1);
  first.append(original.buffer, 16000, 1);
  const saved = first.finalize();
  const snapshot = saved.bytes.slice();
  const resumed = new SessionWavWriter("resume", saved.name);
  assert.equal(resumed.previousDurationMs, 100);
  resumed.append(new Uint8Array(6400).fill(2).buffer, 16000, 1);
  const combined = resumed.finalize();
  assert.equal(resumed.durationMs, 300);
  assert.deepEqual(saved.bytes, snapshot);
  assert.deepEqual(combined.bytes.slice(44, 3244), original);
  assert.deepEqual(combined.bytes.slice(3244), new Uint8Array(6400).fill(2));
  const third = new SessionWavWriter("resume", combined.name);
  third.append(new Uint8Array(3200).fill(3).buffer, 16000, 1);
  assert.equal(third.durationMs, 400);
  assert.equal(saved.exists, true);
  assert.deepEqual(
    saved.bytes,
    snapshot,
    "an upload of the original file must remain readable after another resume",
  );
  assert.equal(
    new DataView(third.finalize().bytes.buffer).getUint32(40, true),
    12800,
  );
});

test("a cancelled resume or incompatible WAV leaves the saved recording intact", () => {
  const first = new SessionWavWriter("cancel");
  first.append(new ArrayBuffer(3200), 16000, 1);
  const saved = first.finalize();
  const snapshot = saved.bytes.slice();
  const resume = new SessionWavWriter("cancel", saved.name);
  resume.closeAndDiscardEmpty();
  assert.equal(resume.file.exists, false);
  assert.deepEqual(saved.bytes, snapshot);
  const stereo = new SessionWavWriter("stereo");
  stereo.append(new ArrayBuffer(3200), 48000, 2);
  const other = stereo.finalize();
  const original = other.bytes.slice();
  assert.throws(
    () => new SessionWavWriter("stereo", other.name),
    /16 kHz mono/,
  );
  assert.deepEqual(other.bytes, original);
});
