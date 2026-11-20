import type {
  BeginDictation,
  DictationError,
  DictationFinish,
  DictationUpdate,
  PreviewConfig,
} from "@anlg/plugin-dictation";

import { DictationRetired, type DictationNative } from "./engine";

/** A native engine that follows the published contract's ordering and staleness rules. */
export class FakeNative implements DictationNative {
  ops = new Map<
    string,
    {
      request: BeginDictation;
      onUpdate: (update: DictationUpdate) => void;
      state: "open" | "finished" | "completed" | "released" | "cancelled";
      finish?: DictationFinish;
      streamed: boolean;
      /** Holds the one microphone until capture has closed, like the native lease. */
      micOpen: boolean;
    }
  >();
  /** What a cancel waits on before its microphone is closed. */
  closing: Promise<void> = Promise.resolve();
  tombstones = new Set<string>();
  records: { id: string; accountId: string; text: string | null }[] = [];
  outcome: (streamed: boolean) => DictationFinish = (streamed) =>
    streamed
      ? { type: "final", text: "hello world", durationMs: 900 }
      : { type: "full", audioPath: "/tmp/full.wav", durationMs: 900 };
  opening: Promise<void> = Promise.resolve();

  private live(id: string) {
    const op = this.ops.get(id);
    if (!op || op.state === "cancelled" || op.state === "released")
      throw new DictationRetired();
    return op;
  }

  get streamedAny() {
    return [...this.ops.values()].some((op) => op.streamed);
  }

  get openOps() {
    return [...this.ops.values()].filter(
      (op) => op.state !== "cancelled" && op.state !== "released",
    ).length;
  }

  begin = async (
    request: BeginDictation,
    onUpdate: (update: DictationUpdate) => void,
  ) => {
    await this.opening;
    if (this.tombstones.has(request.operationId)) throw new DictationRetired();
    if ([...this.ops.values()].some((op) => op.micOpen))
      throw new Error("Voice input is already active.");
    this.ops.set(request.operationId, {
      request,
      onUpdate,
      state: "open",
      streamed: false,
      micOpen: true,
    });
    queueMicrotask(() => onUpdate({ type: "ready" }));
    return {
      operationId: request.operationId,
      generation: this.ops.size,
      recovery: request.recovery,
    };
  };

  attach = async (id: string, _stream: PreviewConfig) => {
    this.live(id).streamed = true;
  };

  finish = async (id: string) => {
    const op = this.live(id);
    op.state = "finished";
    op.micOpen = false;
    op.finish = this.outcome(op.streamed);
    return op.finish;
  };

  complete = async (id: string, batch: string | null) => {
    const op = this.live(id);
    op.state = "completed";
    const finish = op.finish!;
    if (finish.type === "final") return finish.text;
    if (finish.type === "suffix")
      return [finish.committedText, batch].filter(Boolean).join(" ");
    if (finish.type === "full") return batch ?? "";
    return "";
  };

  release = async (id: string, keep: boolean) => {
    const op = this.live(id);
    op.state = "released";
    if (!keep) return null;
    const record = `record-${id}`;
    this.records.push({
      id: record,
      accountId: op.request.accountId,
      text: null,
    });
    return record;
  };

  cancel = async (id: string) => {
    const op = this.ops.get(id);
    if (!op) {
      this.tombstones.add(id);
      return;
    }
    op.state = "cancelled";
    // The operation is gone at once; its microphone closes when capture has stopped.
    await this.closing;
    op.micOpen = false;
  };

  saveRecoveryText = async (accountId: string, id: string, text: string) => {
    const record = this.records.find(
      (entry) => entry.id === id && entry.accountId === accountId,
    );
    if (record) record.text = text;
  };
}

type Ok<T> = { status: "ok"; data: T };
type Failed = { status: "error"; error: DictationError };

async function result<T>(run: () => Promise<T>): Promise<Ok<T> | Failed> {
  try {
    return { status: "ok", data: await run() };
  } catch (error) {
    return {
      status: "error",
      error: {
        code: error instanceof DictationRetired ? "stale" : "invalid",
        message: String(error),
      },
    };
  }
}

/** The engine commands of `@anlg/plugin-dictation`, backed by a fake native engine. */
export function fakeDictationCommands(native: FakeNative) {
  return {
    beginDictation: (
      request: BeginDictation,
      channel: { onmessage: (update: DictationUpdate) => void },
    ) =>
      result(() =>
        native.begin(request, (update) => channel.onmessage(update)),
      ),
    attachDictationStream: (id: string, stream: PreviewConfig) =>
      result(() => native.attach(id, stream).then(() => null)),
    finishDictation: (id: string) => result(() => native.finish(id)),
    completeDictation: (id: string, batch: string | null) =>
      result(() => native.complete(id, batch)),
    releaseDictation: (id: string, keep: boolean) =>
      result(() => native.release(id, keep)),
    cancelDictation: (id: string) =>
      result(() => native.cancel(id).then(() => null)),
    saveDictationRecoveryText: (accountId: string, id: string, text: string) =>
      result(() =>
        native.saveRecoveryText(accountId, id, text).then(() => null),
      ),
    fitDictationText: async (text: string) => ({ text, join: false }),
  };
}
