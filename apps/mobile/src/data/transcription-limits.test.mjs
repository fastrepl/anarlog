import assert from "node:assert/strict";
import test from "node:test";

import { TranscriptionAdmission } from "./transcription-admission.ts";
import {
  assertBoundedTranscriptionResponse,
  boundedSyntheticTokens,
  readBoundedTranscriptionResponse,
} from "./transcription-response.ts";

test("transcription admission limits active work during a burst", async () => {
  const admission = new TranscriptionAdmission(2, 100);
  let active = 0;
  let peak = 0;
  const jobs = Array.from({ length: 50 }, () =>
    admission.schedule(async () => {
      active += 1;
      peak = Math.max(peak, active);
      await new Promise((resolve) => setTimeout(resolve, 1));
      active -= 1;
    }),
  );

  assert.ok(jobs.every((job) => job !== null));
  await Promise.all(jobs);
  assert.equal(peak, 2);
});

test("transcription admission rejects work beyond the queue boundary", async () => {
  const admission = new TranscriptionAdmission(1, 2);
  let releaseFirst;
  const first = admission.schedule(
    () =>
      new Promise((resolve) => {
        releaseFirst = resolve;
      }),
  );
  const second = admission.schedule(async () => {});
  const third = admission.schedule(async () => {});

  assert.ok(first);
  assert.ok(second);
  assert.ok(third);
  assert.equal(
    admission.schedule(async () => {}),
    null,
  );

  await Promise.resolve();
  releaseFirst();
  await Promise.all([first, second, third]);
  const afterDrain = admission.schedule(async () => {});
  assert.ok(afterDrain);
  await afterDrain;
});

test("bounded response reader cancels a chunked response at the limit", async () => {
  let cancelled = false;
  const response = new Response(
    new ReadableStream({
      start(controller) {
        controller.enqueue(new Uint8Array([1, 2, 3]));
        controller.enqueue(new Uint8Array([4, 5, 6]));
      },
      cancel() {
        cancelled = true;
      },
    }),
  );

  await assert.rejects(
    readBoundedTranscriptionResponse(response, 5),
    /response is too large/,
  );
  assert.equal(cancelled, true);
});

test("native response validation counts UTF-8 bytes without copying", () => {
  assert.doesNotThrow(() => assertBoundedTranscriptionResponse("abc", null, 3));
  assert.throws(
    () => assertBoundedTranscriptionResponse("🙂", null, 3),
    /response is too large/,
  );
  assert.throws(
    () => assertBoundedTranscriptionResponse("ok", "4", 3),
    /response is too large/,
  );
});

test("synthetic tokenization stops at the word cap", () => {
  assert.deepEqual(boundedSyntheticTokens(" one  two ", 2), ["one", "two"]);

  const oversizedTranscript = "word ".repeat(500_000);
  assert.throws(
    () => boundedSyntheticTokens(oversizedTranscript, 5),
    /response is too large/,
  );
});
