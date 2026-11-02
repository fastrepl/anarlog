import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { test } from "node:test";

const fixture = (globalThis.recordingRecoveryFixture = {
  files: new Map(),
  sessions: [],
  interruptions: [],
  cataloged: [],
  transcriptionQueue: [],
  analytics: [],
  failures: [],
});

const mocks = {
  "expo-file-system": `
    const fixture = globalThis.recordingRecoveryFixture;
    export const Paths = {document: "/documents"};
    export const FileMode = {ReadWrite: "read-write"};
    export class File {
      constructor(...parts) {
        this.path = parts.map((part) => typeof part === "string" ? part : part.path).join("/");
      }
      get exists() { return fixture.files.has(this.path); }
      get size() { return fixture.files.get(this.path)?.length ?? 0; }
      open() {
        const file = this;
        return {
          offset: 0,
          readBytes(length) {
            const bytes = file.bytes.slice(this.offset, this.offset + length);
            this.offset += bytes.length;
            return bytes;
          },
          writeBytes(bytes) {
            const next = new Uint8Array(Math.max(file.bytes.length, this.offset + bytes.length));
            next.set(file.bytes);
            next.set(bytes, this.offset);
            file.bytes = next;
            this.offset += bytes.length;
          },
          close() {},
        };
      }
      get bytes() { return fixture.files.get(this.path) ?? new Uint8Array(); }
      set bytes(value) { fixture.files.set(this.path, value); }
    }
  `,
  "@/data/audio-catalog": `
    const fixture = globalThis.recordingRecoveryFixture;
    export async function recordSessionCaptureInterruption(id) { fixture.interruptions.push(id); }
    export async function catalogSessionAudio(id, file) { fixture.cataloged.push({id, ...file}); }
  `,
  "@/data/transcribe": `
    const fixture = globalThis.recordingRecoveryFixture;
    export function transcribeSession(id) { fixture.transcriptionQueue.push(id); }
  `,
  "@/db": `
    export async function execute() { return globalThis.recordingRecoveryFixture.sessions.map((id) => ({id})); }
  `,
  "@/lib/analytics": `
    export function captureAnalytics(name, properties) { globalThis.recordingRecoveryFixture.analytics.push({name, properties}); }
  `,
  "@/lib/error-reporting": `
    export function captureOperationalError(error, context) { globalThis.recordingRecoveryFixture.failures.push({error, context}); }
  `,
  "@/live-activity/meeting-recording-activity": `
    export async function clearStaleMeetingRecordingActivities() {}
  `,
};

registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "@/audio/pcm-wav") {
      return {
        url: new URL("./pcm-wav.ts", import.meta.url).href,
        shortCircuit: true,
      };
    }
    if (mocks[specifier]) {
      return {
        url: `data:text/javascript,${encodeURIComponent(mocks[specifier])}`,
        shortCircuit: true,
      };
    }
    return next(specifier, context);
  },
});

const { recoverInterruptedRecordings } =
  await import("./recover-recordings.ts");
const AUDIO_PATH = "/documents/sessions/recovered/audio.wav";

test("repairs and queues a valid interrupted recording for transcription", async () => {
  fixture.files.clear();
  fixture.sessions = ["recovered"];
  fixture.interruptions = [];
  fixture.cataloged = [];
  fixture.transcriptionQueue = [];
  fixture.analytics = [];
  fixture.failures = [];

  const { wavHeader } = await import("./pcm-wav.ts");
  const fileBytes = new Uint8Array(44 + 3_200);
  fileBytes.set(wavHeader(0, 16_000, 1));
  fixture.files.set(AUDIO_PATH, fileBytes);

  const recovered = await recoverInterruptedRecordings(null);

  assert.equal(recovered, 1);
  assert.equal(
    new DataView(fixture.files.get(AUDIO_PATH).buffer).getUint32(40, true),
    3_200,
  );
  assert.deepEqual(fixture.interruptions, ["recovered"]);
  assert.deepEqual(fixture.cataloged, [
    {
      id: "recovered",
      source: "recording",
      filename: "audio.wav",
      contentType: "audio/wav",
    },
  ]);
  assert.deepEqual(fixture.transcriptionQueue, ["recovered"]);
  assert.deepEqual(fixture.analytics, [
    {
      name: "recording_recovered",
      properties: { recording_count: 1 },
    },
  ]);
  assert.deepEqual(fixture.failures, []);
});

test("does not catalog or transcribe an empty interrupted recording", async () => {
  fixture.files.clear();
  fixture.sessions = ["empty"];
  fixture.interruptions = [];
  fixture.cataloged = [];
  fixture.transcriptionQueue = [];

  fixture.files.set("/documents/sessions/empty/audio.wav", new Uint8Array(44));

  assert.equal(await recoverInterruptedRecordings(null), 0);
  assert.deepEqual(fixture.interruptions, []);
  assert.deepEqual(fixture.cataloged, []);
  assert.deepEqual(fixture.transcriptionQueue, []);
});
