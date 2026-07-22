import type { LanguageModel } from "ai";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { type EnhanceEligibilitySkipCode, getEligibility } from "./eligibility";
import {
  type EnhancerNote,
  ensureSummaryDocument,
  replaceSummaryDocumentTemplate,
  updateSummaryDocumentTitleIfCurrent,
} from "./storage";

import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import type { TasksActions } from "~/store/zustand/ai-task/tasks";
import { listenerStore } from "~/store/zustand/listener/instance";
import { getTemplateById } from "~/templates/queries";

type EnhanceResult =
  | { type: "started"; noteId: string }
  | { type: "already_active"; noteId: string }
  | { type: "no_model" };

type QueueEmptySummaryResult =
  | { type: "queued" }
  | { type: "summary_exists"; noteId: string };

type EnhanceOpts = {
  isAuto?: boolean;
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

type TiptapNode = {
  content?: TiptapNode[];
  text?: string;
};

function hasTiptapText(node: TiptapNode): boolean {
  if (typeof node.text === "string" && node.text.trim()) {
    return true;
  }

  return node.content?.some(hasTiptapText) ?? false;
}

function hasSummaryContent(value: unknown): boolean {
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
      return hasTiptapText(parsed);
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
) {
  if (opts?.templateId === null) {
    return undefined;
  }

  if (opts?.templateId) {
    return opts.templateId || undefined;
  }

  return getSelectedTemplateId();
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
  private activeAutoEnhance = new Set<string>();
  private pendingRetries = new Map<string, ReturnType<typeof setTimeout>>();
  private unsubscribe: (() => void) | null = null;
  private eventListeners = new Set<(event: EnhancerEvent) => void>();

  constructor(private deps: EnhancerDeps) {}

  start() {
    this.unsubscribe = listenerStore.subscribe((state) => {
      const { status, sessionId } = state.live;

      if (status === "active" && sessionId) {
        this.activeAutoEnhance.delete(sessionId);
        this.clearRetry(sessionId);
      }
    });
  }

  dispose() {
    this.unsubscribe?.();
    this.unsubscribe = null;
    for (const timer of this.pendingRetries.values()) clearTimeout(timer);
    this.pendingRetries.clear();
    this.activeAutoEnhance.clear();
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

  queueAutoEnhance(sessionId: string) {
    if (this.activeAutoEnhance.has(sessionId)) return;
    this.activeAutoEnhance.add(sessionId);
    void this.tryAutoEnhance(sessionId, 0).catch((error) => {
      this.handleAutoEnhanceError(sessionId, error);
    });
  }

  async queueAutoEnhanceIfSummaryEmpty(
    sessionId: string,
  ): Promise<QueueEmptySummaryResult> {
    const snapshot = await this.loadSession(sessionId);
    const templateId = this.deps.getSelectedTemplateId();
    const existingNote = getAutoEnhancedNote(snapshot, templateId);

    if (existingNote && hasSummaryContent(existingNote.content)) {
      return { type: "summary_exists", noteId: existingNote.id };
    }

    if (!existingNote) {
      const eligibility = getEligibility(snapshot.transcripts);
      if (!eligibility.eligible && eligibility.wordCount > 0) {
        await this.ensureNote(sessionId, templateId);
      }
    }

    this.queueAutoEnhance(sessionId);
    return { type: "queued" };
  }

  private async tryAutoEnhance(sessionId: string, attempt: number) {
    if (!this.activeAutoEnhance.has(sessionId)) return;

    const eligibility = await this.checkEligibility(sessionId);
    if (!this.activeAutoEnhance.has(sessionId)) return;

    if (!eligibility.eligible) {
      if (attempt < 20) {
        const timer = setTimeout(() => {
          this.pendingRetries.delete(sessionId);
          void this.tryAutoEnhance(sessionId, attempt + 1).catch((error) => {
            this.handleAutoEnhanceError(sessionId, error);
          });
        }, 500);
        this.pendingRetries.set(sessionId, timer);
        return;
      }

      this.activeAutoEnhance.delete(sessionId);
      this.emit({
        type: "auto-enhance-skipped",
        sessionId,
        reason: eligibility.reason,
        reasonCode: eligibility.code,
      });
      return;
    }

    const result = await this.enhance(sessionId, { isAuto: true });
    if (!this.activeAutoEnhance.has(sessionId)) return;

    if (result.type === "no_model") {
      this.activeAutoEnhance.delete(sessionId);
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

  private handleAutoEnhanceError(sessionId: string, error: unknown) {
    this.activeAutoEnhance.delete(sessionId);
    this.clearRetry(sessionId);
    const reason = error instanceof Error ? error.message : String(error);
    console.error("[enhancer] auto-enhance failed", error);
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
    const snapshot = await this.loadSession(sessionId);
    const { aiTaskStore } = this.deps;
    for (const note of snapshot.enhancedNotes) {
      aiTaskStore.getState().reset(createTaskId(note.id, "enhance"));
    }
  }

  async enhance(sessionId: string, opts?: EnhanceOpts): Promise<EnhanceResult> {
    const { aiTaskStore, getModel, getLLMConn, getSelectedTemplateId } =
      this.deps;

    const model = getModel();
    if (!model) return { type: "no_model" };

    const snapshot = await this.loadSession(sessionId);
    let templateId = resolveTemplateId(opts, getSelectedTemplateId);
    const targetNote = opts?.targetNoteId
      ? getSessionEnhancedNote(snapshot, opts.targetNoteId)
      : undefined;
    const autoNote =
      !targetNote && opts?.isAuto
        ? getAutoEnhancedNote(snapshot, templateId)
        : undefined;
    if (autoNote) {
      templateId = autoNote.templateId || undefined;
    }

    let note =
      targetNote ??
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

    if (existingTask?.status === "success" && hasSummaryContent(note.content)) {
      return { type: "already_active", noteId: note.id };
    }

    const llmConn = getLLMConn();
    void analyticsCommands
      .event({
        event: "note_enhanced",
        is_auto: opts?.isAuto ?? false,
        llm_provider: llmConn?.providerId,
        llm_model: llmConn?.modelId,
        template_id: templateId,
      })
      .catch((error: unknown) => {
        console.error("[enhancer] failed to record analytics", error);
      });

    void aiTaskStore.getState().generate(enhanceTaskId, {
      model,
      taskType: "enhance",
      args: { sessionId, enhancedNoteId: note.id, templateId },
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
