import { beforeEach, describe, expect, it, vi } from "vitest";

import { useLiveAssistStore } from "~/store/zustand/live-assist";

const hoisted = vi.hoisted(() => ({
  streamLiveAssistSuggestion: vi.fn(),
}));

vi.mock("~/session/insights/live-assist", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/session/insights/live-assist")>()),
  streamLiveAssistSuggestion: hoisted.streamLiveAssistSuggestion,
}));

import {
  enqueueLiveAssistKind,
  registerLiveAssistContextBuilder,
  resetLiveAssistJobs,
} from "./live-assist-job";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function flush() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("live-assist-job", () => {
  const sessionId = "session-1";

  beforeEach(() => {
    hoisted.streamLiveAssistSuggestion.mockReset();
    resetLiveAssistJobs(sessionId);
    useLiveAssistStore.getState().clearSession(sessionId);
  });

  it("resolves a generating card to ready with the returned items", async () => {
    hoisted.streamLiveAssistSuggestion.mockResolvedValue(["Ship the doc."]);
    registerLiveAssistContextBuilder(sessionId, async () => ({
      model: {} as never,
      language: "en",
      sourceText: "You: We should ship the doc.",
    }));

    enqueueLiveAssistKind(sessionId, "catch_up");
    await flush();
    await flush();

    const cards = useLiveAssistStore.getState().cardsBySession[sessionId];
    expect(cards).toHaveLength(1);
    expect(cards?.[0]).toMatchObject({
      kind: "catch_up",
      status: "ready",
      items: ["Ship the doc."],
    });
  });

  it("marks a card as errored when generation fails, without throwing", async () => {
    hoisted.streamLiveAssistSuggestion.mockRejectedValue(new Error("boom"));
    registerLiveAssistContextBuilder(sessionId, async () => ({
      model: {} as never,
      language: "en",
      sourceText: "You: We should ship the doc.",
    }));

    enqueueLiveAssistKind(sessionId, "follow_up");
    await flush();
    await flush();

    const cards = useLiveAssistStore.getState().cardsBySession[sessionId];
    expect(cards?.[0]).toMatchObject({ status: "error", errorMessage: "boom" });
  });

  it("runs at most one kind at a time per session, in FIFO order", async () => {
    const first = deferred<string[]>();
    const second = deferred<string[]>();
    let concurrentCalls = 0;
    let maxConcurrentCalls = 0;
    const order: string[] = [];

    hoisted.streamLiveAssistSuggestion.mockImplementation(
      async ({ kind }: { kind: string }) => {
        order.push(kind);
        concurrentCalls += 1;
        maxConcurrentCalls = Math.max(maxConcurrentCalls, concurrentCalls);
        const result = await (kind === "catch_up"
          ? first.promise
          : second.promise);
        concurrentCalls -= 1;
        return result;
      },
    );
    registerLiveAssistContextBuilder(sessionId, async (kind) => ({
      model: {} as never,
      language: "en",
      sourceText: `text for ${kind}`,
    }));

    enqueueLiveAssistKind(sessionId, "catch_up");
    enqueueLiveAssistKind(sessionId, "action_items");
    await flush();

    expect(order).toEqual(["catch_up"]);
    first.resolve(["Catch up item."]);
    await flush();
    await flush();

    expect(order).toEqual(["catch_up", "action_items"]);
    second.resolve(["Action item."]);
    await flush();
    await flush();

    expect(maxConcurrentCalls).toBe(1);
  });

  it("does not enqueue the same kind twice while it is already pending", async () => {
    const gate = deferred<string[]>();
    hoisted.streamLiveAssistSuggestion.mockReturnValue(gate.promise);
    registerLiveAssistContextBuilder(sessionId, async () => ({
      model: {} as never,
      language: "en",
      sourceText: "text",
    }));

    enqueueLiveAssistKind(sessionId, "catch_up");
    enqueueLiveAssistKind(sessionId, "catch_up");
    await flush();

    expect(hoisted.streamLiveAssistSuggestion).toHaveBeenCalledTimes(1);
    gate.resolve(["item"]);
    await flush();
  });

  it("aborts an in-flight job on reset and drops its card instead of erroring it", async () => {
    let capturedSignal: AbortSignal | undefined;
    hoisted.streamLiveAssistSuggestion.mockImplementation(
      ({ signal }: { signal?: AbortSignal }) =>
        new Promise((_resolve, reject) => {
          capturedSignal = signal;
          signal?.addEventListener("abort", () => reject(new Error("Aborted")));
        }),
    );
    registerLiveAssistContextBuilder(sessionId, async () => ({
      model: {} as never,
      language: "en",
      sourceText: "text",
    }));

    enqueueLiveAssistKind(sessionId, "catch_up");
    await flush();

    expect(
      useLiveAssistStore.getState().cardsBySession[sessionId],
    ).toHaveLength(1);

    resetLiveAssistJobs(sessionId);
    await flush();

    expect(capturedSignal?.aborted).toBe(true);
    expect(
      useLiveAssistStore.getState().cardsBySession[sessionId] ?? [],
    ).toHaveLength(0);
  });
});
