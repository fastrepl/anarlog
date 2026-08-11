import type { LanguageModel } from "ai";

import { type EnhanceEligibilitySkipCode, getEligibility } from "./eligibility";
import {
  discardPendingAutoEnhanceJob,
  type EnhancerNote,
  type PendingAutoEnhanceJob,
  ensurePendingAutoEnhanceDocument,
  ensureSummaryDocument,
  loadPendingAutoEnhanceJobs,
  replaceSummaryDocumentTemplate,
  updateSummaryDocumentTitleIfCurrent,
} from "./storage";

import { trackAnalyticsEvent } from "~/analytics";
import { retryDatabaseLock } from "~/db/retry";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import {
  isRetryableAIError,
  type TasksActions,
} from "~/store/zustand/ai-task/tasks";
import { listenerStore } from "~/store/zustand/listener/instance";
import { getTemplateById } from "~/templates/queries";

type EnhanceResult =
  | { type: "started"; noteId: string }
  | { type: "already_active"; noteId: string }
  | { type: "no_model" }
  | { type: "too_short" };

type QueueEmptySummaryResult =
  | { type: "queued" }
  | { type: "summary_exists"; noteId: string };

export type AutoEnhanceMode = "regenerate" | "if_empty";

type EnhanceOpts = {
  isAuto?: boolean;
  pendingAutoEnhance?: PendingAutoEnhanceJob;
  templateId?: string | null;
  targetNoteId?: string;
  templateTitle?: string;
};

type EnhancerEvent =
  | {
      type: "auto-enhance-skipped";
      sessionId: string;
      reason: string;
      reasonCode: EnhanceEligibilitySkipCode | "error";
    }
  | { type: "auto-enhance-started"; sessionId: string; noteId: string }
  | { type: "auto-enhance-no-model"; sessionId: string };

type EnhancerDeps = {
  aiTaskStore: {
    getState: () => Pick<TasksActions, "generate" | "getState" | "reset">;
  };
  getModel: () => LanguageModel | null;
  getLLMConn: () => { providerId?: string; modelId?: string } | null;
  getSelectedTemplateId: () => string | undefined;
};

const UUID_TITLE_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const ISO_TITLE_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/;
const PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL_MS = 5_000;
const MAX_AUTO_ENHANCE_FAILURES = 8;
const AUTO_ENHANCE_BACKOFF_BASE_MS = 30_000;
const AUTO_ENHANCE_BACKOFF_MAX_MS = 15 * 60_000;
const TEXT_CONTAINER_TYPES = new Set([
  "doc",
  "heading",
  "paragraph",
  "text",
  "codeBlock",
  "blockquote",
  "bulletList",
  "orderedList",
  "listItem",
]);

type TiptapNode = {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: TiptapNode[];
  marks?: Array<{ type?: string; attrs?: Record<string, unknown> }>;
  text?: string;
};

function hasMeaningfulTiptapContent(node: TiptapNode): boolean {
  if (typeof node.text === "string" && node.text.trim()) {
    return true;
  }

  if (!node.type || !TEXT_CONTAINER_TYPES.has(node.type)) {
    return true;
  }

  return node.content?.some(hasMeaningfulTiptapContent) ?? false;
}

function collectTiptapText(node: TiptapNode): string {
  const text = typeof node.text === "string" ? node.text : "";
  return text + (node.content?.map(collectTiptapText).join("") ?? "");
}

function hasSummaryContent(value: unknown, sessionTitle?: string): boolean {
  if (typeof value !== "string") {
    return false;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }

  if (!trimmed.startsWith("{")) {
    return true;
  }

  try {
    const parsed = JSON.parse(trimmed);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      (parsed as { type?: unknown }).type === "doc"
    ) {
      const document = parsed as TiptapNode;
      const blocks = document.content ?? [];
      const firstBlock = blocks[0];
      const firstBlockAttrs = firstBlock?.attrs ?? {};
      const synthesizedTitle =
        sessionTitle?.trim() &&
        firstBlock?.type === "heading" &&
        firstBlockAttrs.level === 1 &&
        Object.keys(firstBlockAttrs).length === 1 &&
        collectTiptapText(firstBlock).trim() === sessionTitle.trim() &&
        !firstBlock.content?.some(
          (child) =>
            child.type !== "text" ||
            !child.text?.trim() ||
            Boolean(child.marks?.length),
        );
      return (synthesizedTitle ? blocks.slice(1) : blocks).some(
        hasMeaningfulTiptapContent,
      );
    }
    return true;
  } catch {
    return true;
  }
}

function shouldHydrateTemplateTitle(
  currentTitle: string | null | undefined,
  templateId: string,
) {
  const title = currentTitle?.trim();
  if (!title) {
    return true;
  }

  return (
    title === "Summary" ||
    title === templateId ||
    UUID_TITLE_RE.test(title) ||
    ISO_TITLE_RE.test(title)
  );
}

function resolveTemplateId(
  opts: EnhanceOpts | undefined,
  getSelectedTemplateId: () => string | undefined,
  memoTemplateId?: string,
) {
  if (opts?.templateId === null) {
    return undefined;
  }

  if (opts?.templateId) {
    return opts.templateId || undefined;
  }

  return memoTemplateId || getSelectedTemplateId();
}

let instance: EnhancerService | null = null;

export function getEnhancerService(): EnhancerService | null {
  return instance;
}

export function initEnhancerService(deps: EnhancerDeps): EnhancerService {
  instance?.dispose();
  instance = new EnhancerService(deps);
  instance.start();
  return instance;
}

export class EnhancerService {
  private activeAutoEnhance = new Map<
    string,
    PendingAutoEnhanceJob | undefined
  >();
  private autoEnhanceFailures = new Map<
    string,
    { attempts: number; nextAttemptAt: number }
  >();
  private pendingRetries = new Map<string, ReturnType<typeof setTimeout>>();
  private pendingResumeTimer: ReturnType<typeof setTimeout> | null = null;
  private unsubscribe: (() => void) | null = null;
  private eventListeners = new Set<(event: EnhancerEvent) => void>();
  private started = false;

  constructor(private deps: EnhancerDeps) {}

  start() {
    if (this.started) return;
    this.started = true;
    this.unsubscribe = listenerStore.subscribe((state) => {
      const { status, sessionId } = state.live;

      if (status === "active" && sessionId) {
        this.activeAutoEnhance.delete(sessionId);
        this.clearRetry(sessionId);
      }
    });
    this.schedulePendingAutoEnhanceResume(0);
  }

  dispose() {
    this.started = false;
    this.unsubscribe?.();
    this.unsubscribe = null;
    for (const timer of this.pendingRetries.values()) clearTimeout(timer);
    this.pendingRetries.clear();
    if (this.pendingResumeTimer) clearTimeout(this.pendingResumeTimer);
    this.pendingResumeTimer = null;
    this.activeAutoEnhance.clear();
    this.autoEnhanceFailures.clear();
    this.eventListeners.clear();
    if (instance === this) instance = null;
  }

  on(listener: (event: EnhancerEvent) => void): () => void {
    this.eventListeners.add(listener);
    return () => this.eventListeners.delete(listener);
  }

  private emit(event: EnhancerEvent) {
    this.eventListeners.forEach((listener) => listener(event));
  }

  async checkEligibility(sessionId: string) {
    const snapshot = await this.loadSession(sessionId);
    return getEligibility(snapshot.transcripts);
  }

  queueAutoEnhance(
    sessionId: string,
    pendingAutoEnhance?: PendingAutoEnhanceJob,
  ) {
    if (this.activeAutoEnhance.has(sessionId)) return;
    this.activeAutoEnhance.set(sessionId, pendingAutoEnhance);
    this.runAutoEnhance(sessionId, 0);
  }

  async queueAutoEnhanceIfSummaryEmpty(
    sessionId: string,
  ): Promise<QueueEmptySummaryResult> {
    return retryDatabaseLock(async () => {
      const snapshot = await this.loadSession(sessionId);
      const templateId = resolveTemplateId(
        undefined,
        this.deps.getSelectedTemplateId,
        snapshot.rawTemplateId,
      );
      const existingNote = getAutoEnhancedNote(snapshot, templateId);

      if (
        existingNote &&
        hasSummaryContent(existingNote.content, snapshot.title)
      ) {
        return { type: "summary_exists", noteId: existingNote.id };
      }

      const eligibility = getEligibility(snapshot.transcripts);
      if (eligibility.eligible) {
        const pendingAutoEnhance = await ensurePendingAutoEnhanceDocument(
          sessionId,
          existingNote ? existingNote.templateId || undefined : templateId,
        );
        this.schedulePendingAutoEnhanceResume();
        this.queueAutoEnhance(sessionId, pendingAutoEnhance);
      } else if (!existingNote && eligibility.wordCount > 0) {
        await this.ensureNote(sessionId, templateId);
        this.queueAutoEnhance(sessionId);
      } else {
        this.queueAutoEnhance(sessionId);
      }

      return { type: "queued" };
    });
  }

  async requestAutoEnhance(sessionId: string, mode: AutoEnhanceMode) {
    if (mode === "regenerate") {
      const pendingAutoEnhance = await retryDatabaseLock(async () => {
        const snapshot = await this.loadSession(sessionId);
        const selectedTemplateId = resolveTemplateId(
          undefined,
          this.deps.getSelectedTemplateId,
          snapshot.rawTemplateId,
        );
        const existingNote = getAutoEnhancedNote(snapshot, selectedTemplateId);
        return ensurePendingAutoEnhanceDocument(
          sessionId,
          existingNote
            ? existingNote.templateId || undefined
            : selectedTemplateId,
        );
      });
      await this.resetEnhanceTasks(sessionId);
      this.activeAutoEnhance.delete(sessionId);
      this.clearRetry(sessionId);
      this.schedulePendingAutoEnhanceResume();
      this.queueAutoEnhance(sessionId, pendingAutoEnhance);
      return;
    }

    await this.queueAutoEnhanceIfSummaryEmpty(sessionId);
  }

  private runAutoEnhance(sessionId: string, attempt: number) {
    void retryDatabaseLock(() => this.tryAutoEnhance(sessionId, attempt)).catch(
      (error) => {
        this.handleAutoEnhanceError(sessionId, error);
      },
    );
  }

  private schedulePendingAutoEnhanceResume(
    delayMs = PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL_MS,
  ) {
    if (!this.started || this.pendingResumeTimer) return;

    this.pendingResumeTimer = setTimeout(() => {
      this.pendingResumeTimer = null;
      void this.resumePendingAutoEnhance();
    }, delayMs);
  }

  private async resumePendingAutoEnhance() {
    try {
      const jobs = await retryDatabaseLock(loadPendingAutoEnhanceJobs);
      if (!this.started) return;

      for (const job of jobs) {
        const live = listenerStore.getState().live;
        if (live.status === "active" && live.sessionId === job.sessionId) {
          continue;
        }
        const failure = this.autoEnhanceFailures.get(this.failureKey(job));
        if (failure && Date.now() < failure.nextAttemptAt) {
          continue;
        }
        console.info("[enhancer] resuming pending auto-enhance", {
          sessionId: job.sessionId,
        });
        this.queueAutoEnhance(job.sessionId, job);
      }

      if (jobs.length > 0) {
        this.schedulePendingAutoEnhanceResume();
      }
    } catch (error) {
      console.error("[enhancer] failed to resume pending auto-enhance", error);
      this.schedulePendingAutoEnhanceResume();
    }
  }

  private async tryAutoEnhance(sessionId: string, attempt: number) {
    if (!this.activeAutoEnhance.has(sessionId)) return;

    const eligibility = await this.checkEligibility(sessionId);
    if (!this.activeAutoEnhance.has(sessionId)) return;

    if (!eligibility.eligible) {
      if (attempt < 20) {
        const timer = setTimeout(() => {
          this.pendingRetries.delete(sessionId);
          this.runAutoEnhance(sessionId, attempt + 1);
        }, 500);
        this.pendingRetries.set(sessionId, timer);
        return;
      }

      const pendingJob = this.activeAutoEnhance.get(sessionId);
      this.activeAutoEnhance.delete(sessionId);
      if (pendingJob) {
        await discardPendingAutoEnhanceJob(pendingJob);
      }
      this.emit({
        type: "auto-enhance-skipped",
        sessionId,
        reason: eligibility.reason,
        reasonCode: eligibility.code,
      });
      return;
    }

    const pendingAutoEnhance = this.activeAutoEnhance.get(sessionId);
    const result = await this.enhance(sessionId, {
      isAuto: true,
      pendingAutoEnhance,
    });
    if (!this.activeAutoEnhance.has(sessionId)) return;

    if (result.type === "too_short") {
      this.activeAutoEnhance.delete(sessionId);
      if (pendingAutoEnhance) {
        await discardPendingAutoEnhanceJob(pendingAutoEnhance);
      }
      return;
    }

    if (result.type === "no_model") {
      this.activeAutoEnhance.delete(sessionId);
      if (pendingAutoEnhance) {
        this.schedulePendingAutoEnhanceResume();
      }
      this.emit({ type: "auto-enhance-no-model", sessionId });
      return;
    }

    this.activeAutoEnhance.delete(sessionId);
    this.emit({
      type: "auto-enhance-started",
      sessionId,
      noteId: result.noteId,
    });
  }

  private failureKey(job: PendingAutoEnhanceJob): string {
    return `${job.sessionId}:${job.generation}`;
  }

  // Retryable failures back off exponentially and give up after a fixed
  // budget; otherwise a persistently failing provider retries every resume
  // tick forever.
  private async recordAutoEnhanceFailure(
    sessionId: string,
    job: PendingAutoEnhanceJob,
  ) {
    const key = this.failureKey(job);
    const attempts = (this.autoEnhanceFailures.get(key)?.attempts ?? 0) + 1;

    if (attempts >= MAX_AUTO_ENHANCE_FAILURES) {
      // Hold the budget entry until the durable job is actually gone. Clearing
      // it first lets the 5s resume tick pick the still-pending job back up
      // with no backoff and restart the attempt count, and leaves retries
      // unbounded if the discard throws.
      this.autoEnhanceFailures.set(key, {
        attempts,
        nextAttemptAt: Date.now() + AUTO_ENHANCE_BACKOFF_MAX_MS,
      });
      await retryDatabaseLock(() => discardPendingAutoEnhanceJob(job));
      this.autoEnhanceFailures.delete(key);
      this.emit({
        type: "auto-enhance-skipped",
        sessionId,
        reason: "Could not generate the summary after repeated attempts.",
        reasonCode: "error",
      });
      return;
    }

    this.autoEnhanceFailures.set(key, {
      attempts,
      nextAttemptAt:
        Date.now() +
        Math.min(
          AUTO_ENHANCE_BACKOFF_BASE_MS * 2 ** (attempts - 1),
          AUTO_ENHANCE_BACKOFF_MAX_MS,
        ),
    });
  }

  private handleAutoEnhanceError(sessionId: string, error: unknown) {
    this.activeAutoEnhance.delete(sessionId);
    this.clearRetry(sessionId);
    const reason = error instanceof Error ? error.message : String(error);
    console.error("[enhancer] auto-enhance failed", error);
    this.schedulePendingAutoEnhanceResume();
    this.emit({
      type: "auto-enhance-skipped",
      sessionId,
      reason,
      reasonCode: "error",
    });
  }

  private clearRetry(sessionId: string) {
    const timer = this.pendingRetries.get(sessionId);
    if (timer) {
      clearTimeout(timer);
      this.pendingRetries.delete(sessionId);
    }
  }

  async resetEnhanceTasks(sessionId: string): Promise<void> {
    await retryDatabaseLock(async () => {
      const snapshot = await this.loadSession(sessionId);
      const { aiTaskStore } = this.deps;
      for (const note of snapshot.enhancedNotes) {
        aiTaskStore.getState().reset(createTaskId(note.id, "enhance"));
      }
    });
  }

  async enhance(sessionId: string, opts?: EnhanceOpts): Promise<EnhanceResult> {
    const { aiTaskStore, getModel, getLLMConn, getSelectedTemplateId } =
      this.deps;

    const model = getModel();
    if (!model) return { type: "no_model" };

    const snapshot = await this.loadSession(sessionId);
    const eligibility = getEligibility(snapshot.transcripts);
    if (!eligibility.eligible && eligibility.code === "transcript_too_short") {
      this.emit({
        type: "auto-enhance-skipped",
        sessionId,
        reason: eligibility.reason,
        reasonCode: eligibility.code,
      });
      return { type: "too_short" };
    }

    let templateId = resolveTemplateId(
      opts,
      getSelectedTemplateId,
      snapshot.rawTemplateId,
    );
    const pendingAutoEnhanceNote = opts?.pendingAutoEnhance
      ? getSessionEnhancedNote(snapshot, opts.pendingAutoEnhance.noteId)
      : undefined;
    const targetNote = opts?.targetNoteId
      ? getSessionEnhancedNote(snapshot, opts.targetNoteId)
      : undefined;
    const autoNote =
      !targetNote && !pendingAutoEnhanceNote && opts?.isAuto
        ? getAutoEnhancedNote(snapshot, templateId)
        : undefined;
    if (pendingAutoEnhanceNote || autoNote) {
      templateId =
        (pendingAutoEnhanceNote ?? autoNote)?.templateId || undefined;
    }

    let note =
      targetNote ??
      pendingAutoEnhanceNote ??
      autoNote ??
      (await this.ensureNoteRecord(sessionId, templateId));
    const enhanceTaskId = createTaskId(note.id, "enhance");
    const existingTask = aiTaskStore.getState().getState(enhanceTaskId);
    if (existingTask?.status === "generating") {
      return { type: "already_active", noteId: note.id };
    }

    if (targetNote) {
      await this.replaceNoteTemplate(
        sessionId,
        targetNote.id,
        templateId,
        opts?.templateTitle,
      );
      note = {
        ...targetNote,
        title: opts?.templateTitle?.trim() || "Summary",
        markdown: "",
        content: "",
        contentFormat: "prosemirror_json",
        templateId: templateId ?? "",
      };
    }

    if (
      existingTask?.status === "success" &&
      hasSummaryContent(note.content, snapshot.title)
    ) {
      return { type: "already_active", noteId: note.id };
    }

    const llmConn = getLLMConn();
    void aiTaskStore
      .getState()
      .generate(enhanceTaskId, {
        model,
        taskType: "enhance",
        args: {
          sessionId,
          enhancedNoteId: note.id,
          templateId,
          ...(opts?.pendingAutoEnhance
            ? {
                pendingAutoEnhance: {
                  generation: opts.pendingAutoEnhance.generation,
                  expectedBody: opts.pendingAutoEnhance.expectedBody,
                  expectedContentFormat:
                    opts.pendingAutoEnhance.expectedContentFormat,
                },
              }
            : {}),
        },
        onComplete: () => {
          trackAnalyticsEvent("note_enhanced", {
            is_auto: opts?.isAuto ?? false,
            llm_provider: llmConn?.providerId ?? "unknown",
            llm_model: llmConn?.modelId ?? "unknown",
            used_template: Boolean(templateId),
          });
          if (templateId) {
            trackAnalyticsEvent("template_applied", {
              entry_point: opts?.isAuto ? "auto_enhance" : "enhance",
            });
          }
        },
      })
      .then(async () => {
        const taskState = aiTaskStore.getState().getState(enhanceTaskId);
        if (taskState?.status === "error") {
          trackAnalyticsEvent("enhancement_failed", {
            is_auto: opts?.isAuto ?? false,
            llm_provider: llmConn?.providerId ?? "unknown",
            failure_stage: "generation",
          });
          if (opts?.pendingAutoEnhance && taskState.error) {
            if (isRetryableAIError(taskState.error)) {
              await this.recordAutoEnhanceFailure(
                sessionId,
                opts.pendingAutoEnhance,
              );
            } else {
              this.autoEnhanceFailures.delete(
                this.failureKey(opts.pendingAutoEnhance),
              );
              await retryDatabaseLock(() =>
                discardPendingAutoEnhanceJob(opts.pendingAutoEnhance!),
              );
            }
          }
        }
      })
      .catch((error) => {
        console.error(
          "[enhancer] failed to finalize auto-enhance task state",
          error,
        );
      });

    return { type: "started", noteId: note.id };
  }

  async ensureNote(sessionId: string, templateId?: string): Promise<string> {
    return (await this.ensureNoteRecord(sessionId, templateId)).id;
  }

  private async ensureNoteRecord(
    sessionId: string,
    templateId?: string,
  ): Promise<EnhancerNote> {
    const note = await ensureSummaryDocument(sessionId, templateId);
    if (templateId) {
      void this.hydrateTemplateTitle(sessionId, note.id, templateId);
    }
    return note;
  }

  private async replaceNoteTemplate(
    sessionId: string,
    noteId: string,
    templateId: string | undefined,
    templateTitle: string | undefined,
  ) {
    const title = templateTitle?.trim() || "Summary";
    await replaceSummaryDocumentTemplate({
      sessionId,
      noteId,
      templateId,
      title,
    });

    if (templateId && !templateTitle?.trim()) {
      void this.hydrateTemplateTitle(sessionId, noteId, templateId);
    }
  }

  private async hydrateTemplateTitle(
    sessionId: string,
    noteId: string,
    templateId: string,
  ): Promise<void> {
    try {
      const template = await getTemplateById(templateId);
      const title = template?.title?.trim();
      if (!title) return;

      const snapshot = await this.loadSession(sessionId);
      const note = getSessionEnhancedNote(snapshot, noteId);
      if (
        !note ||
        note.templateId !== templateId ||
        !shouldHydrateTemplateTitle(note.title, templateId)
      ) {
        return;
      }

      await updateSummaryDocumentTitleIfCurrent({
        sessionId,
        noteId,
        templateId,
        currentTitle: note.title,
        nextTitle: title,
      });
    } catch (error) {
      console.error("[enhancer] failed to hydrate template title", error);
    }
  }

  private async loadSession(sessionId: string) {
    const snapshot = await loadSessionContentSnapshot(sessionId);
    if (!snapshot) {
      throw new Error(`Session ${sessionId} no longer exists`);
    }
    return snapshot;
  }
}

function getSessionEnhancedNote(
  snapshot: SessionContentSnapshot,
  noteId: string,
): EnhancerNote | undefined {
  return snapshot.enhancedNotes.find((note) => note.id === noteId);
}

function getMatchingEnhancedNote(
  snapshot: SessionContentSnapshot,
  templateId?: string,
): EnhancerNote | undefined {
  const normalizedTemplateId = templateId ?? "";
  return snapshot.enhancedNotes.find(
    (note) => note.templateId === normalizedTemplateId,
  );
}

function getAutoEnhancedNote(
  snapshot: SessionContentSnapshot,
  templateId?: string,
): EnhancerNote | undefined {
  return (
    getMatchingEnhancedNote(snapshot, templateId) ??
    [...snapshot.enhancedNotes].sort(
      (left, right) =>
        left.position - right.position || left.id.localeCompare(right.id),
    )[0]
  );
}
