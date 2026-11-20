import { MockLanguageModelV3 } from "ai/test";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type {
  ModelReply,
  PendingRefinement,
  RefineOutcome,
  RefinePlan,
  RefineRequest,
} from "@anlg/plugin-dictation";

import { refineDictation } from "./dictation-refine";

// A stand-in for dictation-core over IPC: plans one call with a short deadline, and maps each
// reply the way the core does, so the outcome shows which reply the adapter produced. `stall`
// makes one command never answer, and reports when it was reached.
const core = vi.hoisted(() => ({
  reachable: true,
  deadlineMs: 50,
  stall: null as null | { at: "prepare" | "resolve"; reached: () => void },
}));

function stalls(at: "prepare" | "resolve") {
  if (core.stall?.at !== at) return false;
  core.stall.reached();
  return true;
}

vi.mock("@anlg/plugin-dictation", () => ({
  commands: {
    prepareDictationRefinement: async (
      request: RefineRequest,
    ): Promise<RefinePlan> => {
      if (!core.reachable) throw new Error("IPC unavailable");
      if (stalls("prepare")) return new Promise(() => {});
      const raw = request.text.trim();
      if (!request.modelReady) {
        return {
          type: "done",
          outcome: request.selection
            ? { type: "editFailed", reason: "noModel" }
            : { type: "raw", text: raw, reason: "noModel" },
        };
      }
      return {
        type: "ask",
        call: { system: "rules", prompt: raw, deadlineMs: core.deadlineMs },
        pending: {
          kind: request.selection ? "edit" : "cleanup",
          raw,
          selection: request.selection,
          before: null,
          instructed: false,
          replacements: [],
        },
      };
    },
    resolveDictationRefinement: async (
      pending: PendingRefinement,
      reply: ModelReply,
    ): Promise<RefineOutcome> => {
      if (!core.reachable) throw new Error("IPC unavailable");
      if (stalls("resolve")) return new Promise(() => {});
      const edit = pending.kind === "edit";
      if (reply.type === "text") {
        return { type: edit ? "edited" : "cleaned", text: reply.text };
      }
      if (reply.type === "cancelled") return { type: "cancelled" };
      return edit
        ? { type: "editFailed", reason: reply.type }
        : { type: "raw", text: pending.raw, reason: reply.type };
    },
  },
}));

function answer(text: string) {
  return {
    content: [{ type: "text" as const, text }],
    finishReason: { unified: "stop" as const, raw: "stop" },
    usage: {
      inputTokens: { total: 4, noCache: 4, cacheRead: 0, cacheWrite: 0 },
      outputTokens: { total: 2, text: 2, reasoning: 0 },
    },
    warnings: [],
  };
}

function replying(text: string) {
  return new MockLanguageModelV3({ doGenerate: async () => answer(text) });
}

// A model that keeps going after it's told to stop, as a native model waiting on its own
// cancellation may: it answers after `ms`, or never.
function unstoppable(ms?: number) {
  let told: AbortSignal | undefined;
  const model = new MockLanguageModelV3({
    doGenerate: ({ abortSignal }) => {
      told = abortSignal;
      return new Promise((resolve) => {
        if (ms === undefined) return;
        setTimeout(() => resolve(answer("Late reply.")), ms);
      });
    },
  });
  return { model, stopped: () => told?.aborted === true };
}

function failing() {
  return new MockLanguageModelV3({
    doGenerate: async () => {
      throw new Error("Ollama isn't answering at http://localhost:11434/v1.");
    },
  });
}

function request(text: string, selection: string | null = null): RefineRequest {
  return {
    operationId: "op-1",
    nowMs: 0,
    text,
    selection,
    policy: { cleanup: true, voiceEdit: true },
    modelReady: true,
    terms: [],
    replacements: [],
    instructions: null,
    before: null,
    context: null,
  };
}

describe("refineDictation", () => {
  beforeEach(() => {
    core.reachable = true;
    core.deadlineMs = 50;
    core.stall = null;
  });

  test("types the raw words at the cleanup deadline, and nothing over a selection, even when the model never stops", async () => {
    const hung = unstoppable();
    await expect(
      refineDictation({ model: hung.model, request: request("um send it") }),
    ).resolves.toEqual({ type: "raw", text: "um send it", reason: "timedOut" });
    expect(hung.stopped()).toBe(true);

    await expect(
      refineDictation({
        model: unstoppable(200).model,
        request: request("late reply"),
      }),
    ).resolves.toEqual({ type: "raw", text: "late reply", reason: "timedOut" });

    await expect(
      refineDictation({
        model: unstoppable().model,
        request: request("make it shorter", "A long line."),
      }),
    ).resolves.toEqual({ type: "editFailed", reason: "timedOut" });
  });

  test("reports a cancelled dictation as cancelled, even when the model doesn't stop or answers anyway", async () => {
    core.deadlineMs = 5_000;
    const controller = new AbortController();
    const hung = unstoppable();
    const pending = refineDictation({
      model: hung.model,
      request: request("um send it"),
      signal: controller.signal,
    });
    setTimeout(() => controller.abort(), 10);
    await expect(pending).resolves.toEqual({ type: "cancelled" });
    expect(hung.stopped()).toBe(true);

    const late = new AbortController();
    const answersAnyway = new MockLanguageModelV3({
      doGenerate: async () => {
        late.abort();
        return answer("Send it.");
      },
    });
    await expect(
      refineDictation({
        model: answersAnyway,
        request: request("um send it"),
        signal: late.signal,
      }),
    ).resolves.toEqual({ type: "cancelled" });

    await expect(
      refineDictation({
        model: replying("Send it."),
        request: request("um send it"),
        signal: AbortSignal.abort(),
      }),
    ).resolves.toEqual({ type: "cancelled" });
  });

  test("reports cancelled at once while the rules are still preparing or resolving the reply", async () => {
    for (const at of ["prepare", "resolve"] as const) {
      const controller = new AbortController();
      core.stall = { at, reached: () => controller.abort() };
      await expect(
        refineDictation({
          model: replying("Send it."),
          request: request("um send it"),
          signal: controller.signal,
        }),
      ).resolves.toEqual({ type: "cancelled" });
    }
  });

  test("never falls back to another route when the selected model fails or is missing", async () => {
    await expect(
      refineDictation({ model: failing(), request: request("um send it") }),
    ).resolves.toEqual({ type: "raw", text: "um send it", reason: "failed" });
    await expect(
      refineDictation({ model: null, request: request("um send it") }),
    ).resolves.toEqual({ type: "raw", text: "um send it", reason: "noModel" });
    await expect(
      refineDictation({
        model: null,
        request: request("make it shorter", "A long line."),
      }),
    ).resolves.toEqual({ type: "editFailed", reason: "noModel" });
    await expect(
      refineDictation({
        model: replying("Send it."),
        request: request("um send it"),
      }),
    ).resolves.toEqual({ type: "cleaned", text: "Send it." });
  });

  test("keeps the words, and never edits a selection, when the rules can't be reached", async () => {
    core.reachable = false;
    await expect(
      refineDictation({
        model: replying("Send it."),
        request: request(" um send it "),
      }),
    ).resolves.toEqual({ type: "raw", text: "um send it", reason: "failed" });
    await expect(
      refineDictation({
        model: replying("<result>Short.</result>"),
        request: request("make it shorter", "A long line."),
      }),
    ).resolves.toEqual({ type: "editFailed", reason: "failed" });
  });
});
