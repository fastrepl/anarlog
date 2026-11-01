import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import { test } from "node:test";

const root = new URL("../", import.meta.url);
const fixture = (globalThis.recorderResumeFixture = {
  duration: 0,
  starts: 0,
  published: [],
  liveCaptures: [],
  batch: [],
  cleanups: [],
  liveResources: new Set(),
});
const mocks = {
  react: `export const useCallback = f => f; export const useRef = current => ({current}); export const useState = value => [value, () => {}];`,
  "react-native": `export const Platform = {OS: 'ios'}; export const PermissionsAndroid = {}; export const Alert = {alert() {}}; export const AppState = {currentState: 'active', addEventListener: () => ({remove() {}})};`,
  "expo-audio": `const f = globalThis.recorderResumeFixture;
    export const getRecordingPermissionsAsync = async () => ({granted: true});
    export const requestRecordingPermissionsAsync = getRecordingPermissionsAsync;
    export async function setAudioModeAsync() {}
    export function useAudioStream({onBuffer}) {
      f.send = (bytes = 3200) => onBuffer({data: new ArrayBuffer(bytes), sampleRate: 16000, channels: 1});
      return {stream: {async start() {f.starts++;}, stop() {}, addListener: () => ({remove() {}})}};
    }`,
  "@/audio/session-wav-writer": `export class SessionWavWriter {}`,
  "@/audio/prepare-session-recording": `export class RecordingResumeError extends Error {}
    export async function prepareSessionRecording() {
      const f = globalThis.recorderResumeFixture;
      if (f.preparing) await f.preparing;
      const writer = {file: {name: 'audio-' + f.starts + '.wav', exists: true}, previousDurationMs: f.duration, durationMs: f.duration,
        append(buffer) {this.durationMs += buffer.byteLength / 32;}, finalize() {}, closeAndDiscardEmpty() {this.file.exists = false;}};
      f.writer = writer;
      return {writer, startedAtMs: 1000};
    }`,
  "@/data/audio-catalog": `export async function recordSessionCaptureStart() {} export async function clearPendingCapture() {}
    export async function catalogSessionAudio(id, file) {const f = globalThis.recorderResumeFixture; f.duration = f.writer.durationMs; f.published.push({...file, duration: f.duration});}`,
  "@/data/live-transcription": `export class SessionLiveTranscription {
    constructor(id, rate, channels, update, capture) {const f = globalThis.recorderResumeFixture; f.liveCaptures.push(capture); f.liveResources.add(this);}
    sendAudio() {} async stop() {const f = globalThis.recorderResumeFixture; f.liveResources.delete(this); return !f.fallback;}
  } export async function markSessionAudioTranscribed() {}`,
  "@/data/transcribe": `import {isMobileSessionCaptureActive} from '${new URL("./capture-lifecycle.ts", import.meta.url).href}';
    export async function transcribeSession(id) {globalThis.recorderResumeFixture.batch.push({id, active: isMobileSessionCaptureActive(id)});}`,
  "@/data/summarize": `export function generateSummaryAfterTranscription() {}`,
  "@/lib/analytics": `export function captureAnalytics() {}`,
  "@/lib/error-reporting": `export function captureOperationalError(error) {throw error;}`,
  "@/lib/use-mount-effect": `export function useMountEffect(f) {const cleanup = f(); if (cleanup) globalThis.recorderResumeFixture.cleanups.push(cleanup);}`,
  "@/live-activity/meeting-recording-activity": `export async function startMeetingRecordingActivity() {} export async function endMeetingRecordingActivity() {}`,
};
registerHooks({
  resolve(specifier, context, next) {
    if (mocks[specifier])
      return {
        url: `data:text/javascript,${encodeURIComponent(mocks[specifier])}`,
        shortCircuit: true,
      };
    if (specifier.startsWith("@/")) {
      const url = new URL(`${specifier.slice(2)}.ts`, root);
      if (existsSync(url)) return { url: url.href, shortCircuit: true };
    }
    return next(specifier, context);
  },
});
const { useSessionRecorder } = await import("./use-session-recorder.ts");
const { getMobileCaptureActive } = await import("./capture-lifecycle.ts");

test("a saved recorder resumes the same note and starts fallback only after capture releases it", async () => {
  const recorder = useSessionRecorder("same-note", false);
  await recorder.start();
  fixture.send();
  assert.equal(await recorder.stop(), "saved");
  await recorder.start();
  fixture.send();
  fixture.fallback = true;
  assert.equal(await recorder.stop(), "saved");
  assert.deepEqual(
    fixture.published.map((file) => file.duration),
    [100, 200],
  );
  assert.deepEqual(fixture.liveCaptures, [
    { startedAtMs: 1000, resuming: false },
    { startedAtMs: 1100, resuming: true },
  ]);
  assert.deepEqual(fixture.batch, [{ id: "same-note", active: false }]);
  assert.equal(getMobileCaptureActive(), false);
});

test("leaving during resume preparation closes the copied audio without starting the microphone", async () => {
  fixture.cleanups = [];
  let prepared;
  fixture.preparing = new Promise((resolve) => {
    prepared = resolve;
  });
  const recorder = useSessionRecorder("cancelled-note", false);
  const starts = fixture.starts;
  const start = recorder.start();
  await new Promise(setImmediate);
  for (const cleanup of fixture.cleanups) cleanup();
  prepared();
  await start;
  await new Promise(setImmediate);
  assert.equal(fixture.starts, starts);
  assert.equal(fixture.writer.file.exists, false);
  assert.equal(getMobileCaptureActive(), false);
});

test("stopping an empty resume closes transcription without replacing the saved audio", async () => {
  const published = fixture.published.length;
  const recorder = useSessionRecorder("empty-resume", false);
  await recorder.start();
  fixture.send(0);
  assert.equal(await recorder.stop(), "saved");
  assert.equal(fixture.liveResources.size, 0);
  assert.equal(fixture.published.length, published);
  assert.equal(getMobileCaptureActive(), false);
});
