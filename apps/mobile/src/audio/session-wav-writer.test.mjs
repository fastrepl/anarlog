import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { test } from "node:test";

const files = (globalThis.wavWriterFiles = new Map());
registerHooks({
  resolve(specifier, context, next) {
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
      export const FileMode = { ReadWrite: "rw" };
      export class Directory { constructor(...parts) { this.path = parts.join("/"); } create() {} }
      export class File {
        exists = false;
        bytes = new Uint8Array(0);
        constructor(directory, filename) { this.path = directory.path + "/" + filename; globalThis.wavWriterFiles.set(this.path, this); }
        get size() { return this.bytes.length; }
        create() { this.exists = true; }
        delete() { this.exists = false; }
        open() {
          const file = this;
          return { offset: 0, writeBytes(bytes) {
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
