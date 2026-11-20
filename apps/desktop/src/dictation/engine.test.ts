import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PreviewConfig, RefineOutcome } from "@anlg/plugin-dictation";

import {
  DictationEngine,
  EDIT_FAILED_MESSAGE,
  NO_SPEECH_MESSAGE,
  type DeliveryOutcome,
  type DictationPlan,
  type DictationTarget,
} from "./engine";
import { FakeNative } from "./test-utils";

const STREAM: PreviewConfig = {
  provider: "deepgram",
  baseUrl: "https://stt.example",
  apiKey: "key",
  params: { model: "nova-3" },
};

function setup(overrides?: {
  transcribe?: (path: string, signal: AbortSignal) => Promise<string>;
  refine?: (text: string, signal: AbortSignal) => Promise<RefineOutcome>;
  deliver?: (isCurrent: () => boolean) => Promise<DeliveryOutcome>;
  stream?: () => Promise<PreviewConfig | null>;
  recovery?: boolean;
  selection?: string | null;
  restoreRaw?: DictationTarget["restoreRaw"];
  learn?: DictationTarget["learn"];
}) {
  const native = new FakeNative();
  const inserted: { text: string; edit: boolean }[] = [];
  const errors: string[] = [];
  const transcripts: string[] = [];
  const uploads: string[] = [];
  // Each operation's destination, released once nothing can use it any more.
  const targets: string[] = [];
  const released: string[] = [];
  const state = { account: "account-1" as string | null };
  const plan: DictationPlan = {
    policy: { provider: "deepgram", model: "nova-3", localOnly: false },
    microphoneDevice: null,
    recovery: overrides?.recovery ?? false,
    ducking: false,
    livePreview: false,
    stream: overrides?.stream ?? (async () => STREAM),
    transcribe:
      overrides?.transcribe ??
      (async (path) => {
        uploads.push(path);
        return "batched words";
      }),
    refine: async ({ text }, signal) =>
      overrides?.refine
        ? overrides.refine(text, signal)
        : { type: "raw", text, reason: "cleanupOff" },
  };
  const target = (operationId: string): DictationTarget => ({
    selection: overrides?.selection ?? null,
    before: null,
    context: null,
    deliver: async (text, edit, isCurrent) => {
      if (overrides?.deliver) return overrides.deliver(isCurrent);
      if (!isCurrent()) return "unchanged";
      inserted.push({ text, edit });
      return "applied";
    },
    restoreRaw: overrides?.restoreRaw,
    learn: overrides?.learn,
    release: () => released.push(operationId),
  });
  const engine = new DictationEngine({
    surface: "system",
    native,
    account: () => state.account,
    prepare: async () => {},
    open: async (operationId) => {
      targets.push(operationId);
      return { plan, target: target(operationId) };
    },
    onView: () => {},
    onError: (message) => errors.push(message),
    onTranscript: (text) => transcripts.push(text),
  });
  const idle = () => vi.waitFor(() => expect(engine.busy).toBe(false));
  return {
    native,
    engine,
    inserted,
    errors,
    transcripts,
    uploads,
    state,
    idle,
    targets,
    released,
  };
}

async function settle() {
  for (let i = 0; i < 10; i++) await Promise.resolve();
}

describe("dictation engine", () => {
  beforeEach(() => vi.useRealTimers());
  afterEach(() => vi.useRealTimers());

  it("keeps a short tap on the device and inserts nothing", async () => {
    const { native, engine, inserted, uploads, idle, targets, released } =
      setup();
    engine.prime();
    await settle();
    engine.unprime();
    await idle();

    expect(native.streamedAny).toBe(false);
    expect(uploads).toEqual([]);
    expect(inserted).toEqual([]);
    expect(native.openOps).toBe(0);
    await vi.waitFor(() => expect(released).toEqual(targets));
  });

  it("starts a double-tap latch right after the cancelled tap", async () => {
    const { native, engine, inserted, idle } = setup();
    engine.prime();
    await settle();
    engine.unprime();
    engine.start(true);
    await vi.waitFor(() => expect(native.streamedAny).toBe(true));
    engine.stop();
    await idle();

    expect(inserted).toEqual([{ text: "hello world", edit: false }]);
    expect(native.openOps).toBe(0);
  });

  it("waits for the tap's microphone to close before a double-tap latch opens it again", async () => {
    const { native, engine, inserted, errors, idle } = setup();
    engine.prime();
    await settle();
    let closed!: () => void;
    native.closing = new Promise((resolve) => {
      closed = resolve;
    });
    engine.unprime();
    engine.start(true);
    await settle();
    closed();
    await vi.waitFor(() => expect(native.streamedAny).toBe(true));
    engine.stop();
    await idle();

    expect(errors).toEqual([]);
    expect(inserted).toEqual([{ text: "hello world", edit: false }]);
  });

  it("closes a primed microphone whose press never resolves", async () => {
    vi.useFakeTimers();
    const { native, engine } = setup();
    engine.prime();
    await vi.advanceTimersByTimeAsync(2_000);

    expect(engine.busy).toBe(false);
    expect(native.openOps).toBe(0);
    expect(native.streamedAny).toBe(false);
  });

  it("delivers a finalized stream once and releases its files", async () => {
    const { native, engine, inserted, uploads, idle, targets, released } =
      setup();
    engine.prime();
    engine.start(false);
    await vi.waitFor(() => expect(native.streamedAny).toBe(true));
    engine.stop();
    await idle();

    expect(inserted).toEqual([{ text: "hello world", edit: false }]);
    expect(uploads).toEqual([]);
    expect(native.openOps).toBe(0);
    // Nothing to restore or learn: the destination ends with the delivery.
    expect(released).toEqual(targets);
  });

  it("replaces preview words with a full batch when the stream cannot prove completeness", async () => {
    const { engine, inserted, uploads, idle } = setup({
      stream: async () => null,
    });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();

    expect(uploads).toEqual(["/tmp/full.wav"]);
    expect(inserted).toEqual([{ text: "batched words", edit: false }]);
  });

  it("appends only the suffix batch to committed stream words", async () => {
    const { native, engine, inserted, uploads, idle } = setup();
    native.outcome = () => ({
      type: "suffix",
      committedText: "hello",
      audioPath: "/tmp/suffix.wav",
      durationMs: 900,
    });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();

    expect(uploads).toEqual(["/tmp/suffix.wav"]);
    expect(inserted).toEqual([{ text: "hello batched words", edit: false }]);
  });

  it("drops a transcription that outlives its account without inserting or keeping it", async () => {
    let finishBatch!: (text: string) => void;
    const { native, engine, inserted, errors, state, idle } = setup({
      recovery: true,
      stream: async () => null,
      transcribe: () =>
        new Promise<string>((resolve) => {
          finishBatch = resolve;
        }),
    });
    engine.start(true);
    await settle();
    engine.stop();
    await vi.waitFor(() => expect(finishBatch).toBeDefined());
    state.account = "account-2";
    finishBatch("late words");
    await idle();

    expect(inserted).toEqual([]);
    expect(errors).toEqual([]);
    expect(native.records).toEqual([]);
    expect(native.openOps).toBe(0);
  });

  it("closes a microphone that is still opening when the press is cancelled", async () => {
    const { native, engine, inserted, idle } = setup();
    let open!: () => void;
    native.opening = new Promise<void>((resolve) => {
      open = resolve;
    });
    engine.start(false);
    await settle();
    engine.cancel();
    open();
    await idle();

    expect(native.ops.size).toBe(0);
    expect(inserted).toEqual([]);
  });

  it("never inserts after the surface is disposed during cleanup", async () => {
    let finishRefine!: (outcome: RefineOutcome) => void;
    const { engine, inserted } = setup({
      refine: () =>
        new Promise<RefineOutcome>((resolve) => {
          finishRefine = resolve;
        }),
    });
    engine.start(true);
    await settle();
    engine.stop();
    await vi.waitFor(() => expect(finishRefine).toBeDefined());
    const disposed = engine.dispose();
    finishRefine({ type: "cleaned", text: "Hello world." });
    await disposed;

    expect(inserted).toEqual([]);
  });

  it("keeps the cleaned text for explicit recovery when the field refused it", async () => {
    const { native, engine, inserted, errors, idle } = setup({
      recovery: true,
      deliver: async () => "unchanged",
      refine: async () => ({ type: "cleaned", text: "Hello, world." }),
    });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();

    expect(inserted).toEqual([]);
    expect(native.records).toEqual([
      expect.objectContaining({
        accountId: "account-1",
        text: "Hello, world.",
      }),
    ]);
    expect(errors[0]).toMatch(/saved in Settings > Dictation/u);
  });

  it("keeps audio for retry when batch transcription fails", async () => {
    const { native, engine, inserted, errors, idle } = setup({
      recovery: true,
      stream: async () => null,
      transcribe: async () => {
        throw new Error("Network unavailable.");
      },
    });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();

    expect(inserted).toEqual([]);
    expect(native.records).toEqual([
      expect.objectContaining({ accountId: "account-1", text: null }),
    ]);
    expect(errors[0]).toMatch(/^Network unavailable\. The recording is saved/u);
  });

  it.each([
    ["the model cannot edit it", "editFailed", EDIT_FAILED_MESSAGE, []],
    ["it changed before the write", "unchanged", EDIT_FAILED_MESSAGE, []],
    // It may have landed: never promised unchanged, offered for copying, not kept.
    ["the write may have landed", "uncertain", /Couldn't confirm/u, ["Final."]],
  ] as const)(
    "keeps a voice edit's rewrite of the selection out of recovery when %s",
    async (_case, result, error, shown) => {
      const { native, engine, inserted, errors, transcripts, idle } = setup({
        recovery: true,
        selection: "draft text",
        refine: async () =>
          result === "editFailed"
            ? { type: "editFailed", reason: "timedOut" }
            : { type: "edited", text: "Final." },
        deliver: async () => result as DeliveryOutcome,
      });
      engine.start(true);
      await settle();
      engine.stop();
      await idle();

      expect(inserted).toEqual([]);
      expect(errors).toEqual([expect.stringMatching(error)]);
      expect(native.records).toEqual([]);
      expect(transcripts).toEqual(shown);
    },
  );

  it("reports silence without inserting", async () => {
    const { native, engine, inserted, errors, idle } = setup();
    native.outcome = () => ({ type: "silent", durationMs: 400 });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();

    expect(inserted).toEqual([]);
    expect(errors).toEqual([NO_SPEECH_MESSAGE]);
  });

  it("puts back the raw words when the shortcut is tapped right after a cleaned insert", async () => {
    const restored: string[] = [];
    let finishRestore!: () => void;
    const { engine, idle, targets, released } = setup({
      refine: async () => ({ type: "cleaned", text: "Hello, world." }),
      restoreRaw: (raw) =>
        new Promise<void>((resolve) => {
          restored.push(raw);
          finishRestore = resolve;
        }),
    });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();
    const [delivered] = targets;
    expect(released).toEqual([]);
    // A tap is a press: the shortcut primes, then reports the tap.
    engine.prime();
    await settle();
    engine.unprime();
    engine.tap();
    engine.tap();

    expect(restored).toEqual(["hello world"]);
    // The tap's own destination ends with it; the delivered one only once restored.
    await vi.waitFor(() => expect(released).toEqual([targets[1]]));
    finishRestore();
    await vi.waitFor(() => expect(released).toEqual([targets[1], delivered]));
  });

  it("neither restores nor learns from a dictation cancelled after its write was sent", async () => {
    const restored: string[] = [];
    const learned: string[] = [];
    const { engine, idle, targets, released } = setup({
      refine: async () => ({ type: "cleaned", text: "Hello, world." }),
      // Escape arrives once the native write is already on its way: it may have landed.
      deliver: async () => {
        engine.cancel();
        return "applied";
      },
      restoreRaw: async (raw) => {
        restored.push(raw);
      },
      learn: async (text) => {
        learned.push(text);
      },
    });
    engine.start(true);
    await settle();
    engine.stop();
    await idle();
    engine.tap();

    expect(restored).toEqual([]);
    expect(learned).toEqual([]);
    await vi.waitFor(() => expect(released).toEqual(targets));
  });
});
