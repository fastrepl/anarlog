import type {
  BeginDictation,
  ContextSnapshot,
  DictationFinish,
  DictationPolicy,
  DictationSurface,
  DictationTicket,
  DictationUpdate,
  PreviewConfig,
  RefineOutcome,
} from "@anlg/plugin-dictation";

export type EnginePhase = "idle" | "starting" | "recording" | "transcribing";
export type DeliveryOutcome = "applied" | "unchanged" | "uncertain";

// Finishing waits this long for a stream attach still fetching its token.
const ATTACH_GRACE_MS = 3_000;
// A tap of the shortcut this soon after a cleaned-up dictation lands puts back the raw words.
const SWAP_TO_RAW_MS = 15_000;
const PRIMED_WITHOUT_START_MS = 2_000;

export const NO_SPEECH_MESSAGE = "No speech detected. Please try again.";
export const EDIT_FAILED_MESSAGE =
  "Couldn't edit the selection. It was left unchanged.";

/** Thrown when an operation was cancelled, superseded or outlived its account or surface. */
export class DictationRetired extends Error {
  constructor() {
    super("Dictation ended");
  }
}

export type DictationNative = {
  begin: (
    request: BeginDictation,
    onUpdate: (update: DictationUpdate) => void,
  ) => Promise<DictationTicket>;
  attach: (operationId: string, stream: PreviewConfig) => Promise<void>;
  finish: (operationId: string) => Promise<DictationFinish>;
  complete: (operationId: string, batchText: string | null) => Promise<string>;
  release: (operationId: string, keep: boolean) => Promise<string | null>;
  cancel: (operationId: string) => Promise<void>;
  saveRecoveryText: (
    accountId: string,
    id: string,
    text: string,
  ) => Promise<void>;
};

/** The provider route, options and text rules captured once when an operation opens. */
export type DictationPlan = {
  policy: DictationPolicy;
  microphoneDevice: string | null;
  recovery: boolean;
  ducking: boolean;
  livePreview: boolean;
  stream: () => Promise<PreviewConfig | null>;
  transcribe: (audioPath: string, signal: AbortSignal) => Promise<string>;
  refine: (
    request: {
      operationId: string;
      text: string;
      selection: string | null;
      before: string | null;
      context: ContextSnapshot | null;
    },
    signal: AbortSignal,
  ) => Promise<RefineOutcome>;
};

/** Where this operation's text goes, captured when it opens. */
export type DictationTarget = {
  /** Text a voice edit rewrites; null for plain dictation. */
  selection: string | null;
  /** In-app text before the caret for cleanup to continue; never external text. */
  before: string | null;
  context: ContextSnapshot | null;
  /** Writes once; `isCurrent` must be checked right before the write. */
  deliver: (
    text: string,
    edit: boolean,
    isCurrent: () => boolean,
  ) => Promise<DeliveryOutcome>;
  /** Optional learning from the person's edits to this delivered dictation only. */
  learn?: (
    inserted: string,
    isCurrent: () => boolean,
    signal: AbortSignal,
  ) => Promise<void>;
  /** Puts back the raw words over a cleaned-up delivery, if the field still proves it. */
  restoreRaw?: (raw: string) => Promise<void>;
  /** Ends every use of the destination; called once, after delivery and any restore or learning. */
  release?: () => void;
};

export type DictationView = {
  operationId: string;
  phase: EnginePhase;
  amplitude: number;
  text: string;
  partial: string;
  livePreview: boolean;
  previewUnavailable: boolean;
};

export type DictationMetrics = {
  source: DictationFinish["type"];
  refined: RefineOutcome["type"];
  delivery: DeliveryOutcome;
  stopToTextMs: number;
  transcriptMs: number;
  refineMs: number;
  insertMs: number;
};

export type DictationEnginePorts = {
  surface: DictationSurface;
  native: DictationNative;
  account: () => string | null;
  /** Checks that must pass before the microphone opens; throws a user-facing error. */
  prepare: (signal: AbortSignal) => Promise<void>;
  open: (
    operationId: string,
    signal: AbortSignal,
  ) => Promise<{ plan: DictationPlan; target: DictationTarget }>;
  /** Shows the surface's own UI once the dictation really starts. */
  present?: (operationId: string, signal: AbortSignal) => Promise<void>;
  onView: (view: DictationView | null) => void;
  onError: (message: string) => void;
  onTranscript?: (text: string) => void;
  onMeasured?: (metrics: DictationMetrics) => void;
  onRecoveryKept?: () => void;
};

/** What a delivered dictation may still do with its target: put back the raw words once, learn once. */
type AfterDelivery = {
  accountId: string;
  target: DictationTarget;
  at: number;
  raw: string | null;
  restoring: boolean;
  learning: AbortController | null;
};

class Operation {
  readonly id = crypto.randomUUID();
  readonly controller = new AbortController();
  confirmed = false;
  latched = false;
  stopping = false;
  cancelled = false;
  ended = false;
  captureFailed = false;
  /** Settles once a cancelled operation's microphone is closed natively. */
  closed: Promise<void> = Promise.resolve();
  view: DictationView;
  private resolveStop!: () => void;
  private resolveStart!: () => void;
  readonly stopped = new Promise<void>((resolve) => {
    this.resolveStop = resolve;
  });
  readonly started = new Promise<void>((resolve) => {
    this.resolveStart = resolve;
  });

  constructor(readonly accountId: string) {
    this.view = {
      operationId: this.id,
      phase: "starting",
      amplitude: 0,
      text: "",
      partial: "",
      livePreview: false,
      previewUnavailable: false,
    };
  }

  confirm(latched: boolean) {
    this.confirmed = true;
    this.latched ||= latched;
    this.resolveStart();
  }

  stop() {
    if (this.stopping) return;
    this.stopping = true;
    this.resolveStop();
  }

  cancel() {
    this.cancelled = true;
    this.controller.abort();
    this.resolveStart();
    this.resolveStop();
  }
}

/**
 * One dictation at a time for one surface. Every asynchronous step is checked against the
 * operation's account, cancellation and the surface's disposal; a late result neither uploads
 * nor inserts. Exactly one delivery is attempted per operation.
 */
export class DictationEngine {
  private op: Operation | null = null;
  private running: Promise<void> = Promise.resolve();
  private disposed = false;
  private after: AfterDelivery | null = null;

  constructor(private readonly ports: DictationEnginePorts) {}

  /** A cancelled operation still tearing down does not block the next one; it runs after. */
  get busy() {
    return this.op !== null && !this.op.ended && !this.op.cancelled;
  }

  /** Opens the microphone ahead of a press that may become a dictation; shows nothing yet. */
  prime() {
    if (this.disposed || this.busy) return;
    const op = this.begin();
    if (!op) return;
    // A press whose outcome never arrives closes its microphone by itself.
    setTimeout(() => {
      if (!op.confirmed) this.cancelOp(op);
    }, PRIMED_WITHOUT_START_MS);
  }

  /** Cancels a primed operation that never started (a tap or a chord). */
  unprime() {
    const op = this.op;
    if (op && !op.confirmed) this.cancelOp(op);
  }

  start(latched: boolean) {
    if (this.disposed) return;
    const op = this.busy ? this.op! : this.begin();
    if (!op || op.confirmed) return;
    // Only a real dictation ends the previous one's learning read and raw swap; a tap's
    // own prime must not, or the tap could never swap.
    this.endAfter();
    op.confirm(latched);
    this.emit(op, { phase: op.view.phase });
  }

  latch() {
    const op = this.op;
    if (op?.confirmed && !op.stopping) op.latched = true;
  }

  /** Toggle for a single press: start hands-free, or finish the running dictation. */
  toggle() {
    const op = this.op;
    if (op?.confirmed && this.busy) {
      if (!op.stopping) op.stop();
      return;
    }
    this.start(true);
  }

  stop() {
    const op = this.op;
    if (op?.confirmed) op.stop();
  }

  cancel() {
    this.endAfter();
    if (this.op) this.cancelOp(this.op);
  }

  /** A lone tap right after a cleaned-up dictation puts back the raw words. */
  tap() {
    const after = this.after;
    const raw = after?.raw;
    if (!after || !raw) return;
    after.raw = null;
    const restoreRaw = after.target.restoreRaw;
    if (
      !restoreRaw ||
      this.busy ||
      Date.now() - after.at > SWAP_TO_RAW_MS ||
      this.ports.account() !== after.accountId
    ) {
      this.settleAfter(after);
      return;
    }
    after.learning?.abort();
    after.restoring = true;
    void restoreRaw(raw)
      .catch(() => {})
      .finally(() => {
        after.restoring = false;
        this.settleAfter(after);
      });
  }

  async dispose() {
    this.disposed = true;
    this.cancel();
    await this.running;
  }

  private begin() {
    const accountId = this.ports.account();
    if (!accountId) return null;
    const op = new Operation(accountId);
    this.op = op;
    const previous = this.running;
    this.running = previous.then(() => this.run(op));
    return op;
  }

  private cancelOp(op: Operation) {
    if (op.cancelled) return;
    op.cancel();
    // Before `begin` resolves this leaves a tombstone, so the opening microphone closes.
    op.closed = this.ports.native.cancel(op.id).catch(() => {});
  }

  private isCurrent(op: Operation) {
    return (
      this.op === op &&
      !op.cancelled &&
      !this.disposed &&
      this.ports.account() === op.accountId
    );
  }

  private check(op: Operation) {
    if (!this.isCurrent(op)) throw new DictationRetired();
  }

  private emit(op: Operation, patch: Partial<DictationView>) {
    op.view = { ...op.view, ...patch };
    if (this.op === op && op.confirmed && !op.cancelled)
      this.ports.onView(op.view);
  }

  private onUpdate(
    op: Operation,
    plan: DictationPlan,
    update: DictationUpdate,
  ) {
    if (!this.isCurrent(op)) return;
    switch (update.type) {
      case "ready":
        if (!op.stopping) this.emit(op, { phase: "recording" });
        return;
      case "amplitude":
        if (!op.stopping) this.emit(op, { amplitude: update.amplitude });
        return;
      case "transcript":
        if (plan.livePreview)
          this.emit(op, { text: update.text, partial: update.partial });
        return;
      case "previewUnavailable":
        if (plan.livePreview)
          this.emit(op, { previewUnavailable: true, partial: "" });
        return;
      case "captureEnded":
        if (update.reason === "failed") op.captureFailed = true;
        // A primed operation that loses its microphone before starting just ends.
        if (!op.confirmed) this.cancelOp(op);
        else op.stop();
        return;
    }
  }

  private async attach(op: Operation, plan: DictationPlan) {
    try {
      // No audio leaves the device until the press is confirmed as a dictation.
      await op.started;
      if (!this.isCurrent(op)) return;
      const stream = await plan.stream();
      if (!this.isCurrent(op)) return;
      if (!stream) {
        if (plan.livePreview) this.emit(op, { previewUnavailable: true });
        return;
      }
      await this.ports.native.attach(op.id, stream);
    } catch {
      if (this.isCurrent(op) && plan.livePreview)
        this.emit(op, { previewUnavailable: true, partial: "" });
    }
  }

  private async run(op: Operation) {
    const { ports } = this;
    const signal = op.controller.signal;
    let ticket: DictationTicket | null = null;
    let keep = false;
    let keptText: string | null = null;
    let target: DictationTarget | null = null;
    let handedOver = false;
    try {
      await ports.prepare(signal);
      this.check(op);
      const opened = await ports.open(op.id, signal);
      target = opened.target;
      const { plan } = opened;
      this.check(op);
      this.emit(op, { livePreview: plan.livePreview });
      ticket = await ports.native.begin(
        {
          operationId: op.id,
          surface: ports.surface,
          accountId: op.accountId,
          microphoneDevice: plan.microphoneDevice,
          policy: plan.policy,
          stream: null,
          recovery: plan.recovery,
          ducking: plan.ducking,
        },
        (update) => this.onUpdate(op, plan, update),
      );
      this.check(op);
      const attaching = this.attach(op, plan);
      await op.started;
      this.check(op);
      await ports.present?.(op.id, signal);
      this.check(op);
      await op.stopped;
      this.check(op);

      this.emit(op, { phase: "transcribing", amplitude: 0 });
      const stoppedAt = Date.now();
      await Promise.race([attaching, delay(ATTACH_GRACE_MS)]);
      const finish = await ports.native.finish(op.id);
      this.check(op);
      let batch: string | null = null;
      if (finish.type === "suffix" || finish.type === "full") {
        try {
          batch = (await plan.transcribe(finish.audioPath, signal)).trim();
        } catch (error) {
          this.check(op);
          keep = ticket.recovery;
          throw new Error(
            keep
              ? `${errorMessage(error)} The recording is saved in Settings > Dictation.`
              : errorMessage(error),
          );
        }
        this.check(op);
      }
      const raw =
        finish.type === "silent"
          ? ""
          : (await ports.native.complete(op.id, batch)).trim();
      this.check(op);
      if (!raw)
        throw new Error(
          op.captureFailed
            ? "Recording stopped unexpectedly. Please try again."
            : NO_SPEECH_MESSAGE,
        );

      const transcriptAt = Date.now();
      const outcome = await plan.refine(
        {
          operationId: op.id,
          text: raw,
          selection: target.selection,
          before: target.before,
          context: target.context,
        },
        signal,
      );
      this.check(op);
      if (outcome.type === "cancelled") throw new DictationRetired();
      if (outcome.type === "empty") throw new Error(NO_SPEECH_MESSAGE);
      if (outcome.type === "editFailed") throw new Error(EDIT_FAILED_MESSAGE);

      const text = outcome.text;
      // A voice edit's text is a rewrite of the selection: never offered or kept unless it
      // may have landed.
      const edit = outcome.type === "edited";
      if (!edit) ports.onTranscript?.(text);
      const refinedAt = Date.now();
      const delivery = await target.deliver(text, edit, () =>
        this.isCurrent(op),
      );
      ports.onMeasured?.({
        source: finish.type,
        refined: outcome.type,
        delivery,
        stopToTextMs: Date.now() - stoppedAt,
        transcriptMs: transcriptAt - stoppedAt,
        refineMs: refinedAt - transcriptAt,
        insertMs: Date.now() - refinedAt,
      });
      if (delivery === "unchanged") {
        if (edit) throw new Error(EDIT_FAILED_MESSAGE);
        keep = ticket.recovery;
        keptText = text;
        throw new Error(
          keep
            ? "Couldn't insert the text. It's saved in Settings > Dictation."
            : "Couldn't insert the text. Copy it from Settings > Dictation.",
        );
      }
      if (edit && this.isCurrent(op)) ports.onTranscript?.(text);
      if (delivery === "uncertain")
        throw new Error(
          "Couldn't confirm the text was inserted. Check the field before dictating again.",
        );
      // Text that landed for a dictation ended meanwhile is neither restored nor learned from.
      this.check(op);
      handedOver = true;
      this.afterDelivery(op, target, raw, text, outcome.type === "cleaned");
    } catch (error) {
      if (
        !(error instanceof DictationRetired) &&
        op.confirmed &&
        this.isCurrent(op)
      )
        ports.onError(errorMessage(error));
    } finally {
      op.ended = true;
      if (!handedOver) target?.release?.();
      if (ticket && this.isCurrent(op)) {
        await this.release(op, keep && ticket.recovery, keptText);
      } else if (ticket && !op.cancelled) {
        // Outlived its account or surface: nothing is kept or uploaded.
        this.cancelOp(op);
      }
      if (this.op === op) {
        this.op = null;
        if (op.confirmed) ports.onView(null);
      }
      // The next operation opens the microphone only once this one has closed it.
      await op.closed;
    }
  }

  private async release(op: Operation, keep: boolean, text: string | null) {
    try {
      const record = await this.ports.native.release(op.id, keep);
      if (!record) return;
      if (text)
        await this.ports.native
          .saveRecoveryText(op.accountId, record, text)
          .catch(() => {});
      this.ports.onRecoveryKept?.();
    } catch {
      // Release deletes the operation's files even when keeping fails.
    }
  }

  private afterDelivery(
    op: Operation,
    target: DictationTarget,
    raw: string,
    text: string,
    cleaned: boolean,
  ) {
    this.endAfter();
    const after: AfterDelivery = {
      accountId: op.accountId,
      target,
      at: Date.now(),
      raw: cleaned && target.restoreRaw && raw !== text ? raw : null,
      restoring: false,
      learning: null,
    };
    this.after = after;
    if (target.learn) {
      const controller = new AbortController();
      after.learning = controller;
      const isCurrent = () =>
        !controller.signal.aborted &&
        !this.disposed &&
        this.ports.account() === op.accountId;
      void target
        .learn(text, isCurrent, controller.signal)
        .catch(() => {})
        .finally(() => {
          if (after.learning === controller) after.learning = null;
          this.settleAfter(after);
        });
    }
    if (after.raw)
      setTimeout(() => this.settleAfter(after), SWAP_TO_RAW_MS + 1);
    this.settleAfter(after);
  }

  /** Releases the target once its raw swap and learning read can no longer happen. */
  private settleAfter(after: AfterDelivery) {
    if (this.after !== after) return;
    const restorable =
      after.raw !== null && Date.now() - after.at <= SWAP_TO_RAW_MS;
    if (!restorable && !after.restoring && !after.learning) this.endAfter();
  }

  private endAfter() {
    const after = this.after;
    if (!after) return;
    this.after = null;
    after.learning?.abort();
    after.target.release?.();
  }
}

function delay(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

export function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  )
    return error.message;
  return String(error);
}
