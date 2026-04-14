import type { LanguageModel } from "ai";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { getEligibility } from "./eligibility";

import type { Store as MainStore } from "~/store/tinybase/store/main";
import { INDEXES } from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import type { TasksActions } from "~/store/zustand/ai-task/tasks";
import { listenerStore } from "~/store/zustand/listener/instance";
import { getTemplateById } from "~/templates/queries";

type EnhanceResult =
  | { type: "started"; noteId: string }
  | { type: "already_active"; noteId: string }
  | { type: "no_model" };

type EnhanceOpts = {
  isAuto?: boolean;
  templateId?: string;
};

type EnhancerEvent =
  | { type: "auto-enhance-skipped"; sessionId: string; reason: string }
  | { type: "auto-enhance-started"; sessionId: string; noteId: string }
  | { type: "auto-enhance-no-model"; sessionId: string };

type EnhancerDeps = {
  mainStore: MainStore;
  indexes: { getSliceRowIds: (indexId: string, sliceId: string) => string[] };
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
    this.eventListeners.forEach((fn) => fn(event));
  }

  checkEligibility(sessionId: string) {
    const transcriptIds = this.getTranscriptIds(sessionId);
    return getEligibility(
      transcriptIds.length > 0,
      transcriptIds,
      this.deps.mainStore,
    );
  }

  queueAutoEnhance(sessionId: string) {
    if (this.activeAutoEnhance.has(sessionId)) return;
    this.activeAutoEnhance.add(sessionId);
    this.tryAutoEnhance(sessionId, 0);
  }

  private tryAutoEnhance(sessionId: string, attempt: number) {
    const eligibility = this.checkEligibility(sessionId);
    if (!eligibility.eligible) {
      if (attempt < 20) {
        const timer = setTimeout(() => {
          this.pendingRetries.delete(sessionId);
          this.tryAutoEnhance(sessionId, attempt + 1);
        }, 500);
        this.pendingRetries.set(sessionId, timer);
        return;
      }

      this.activeAutoEnhance.delete(sessionId);
      this.emit({
        type: "auto-enhance-skipped",
        sessionId,
        reason: eligibility.reason,
      });
      return;
    }

    const result = this.enhance(sessionId, { isAuto: true });

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

  private clearRetry(sessionId: string) {
    const timer = this.pendingRetries.get(sessionId);
    if (timer) {
      clearTimeout(timer);
      this.pendingRetries.delete(sessionId);
    }
  }

  // Reset enhance task states so auto-enhance can re-run after transcript redo.
  // Without this, tasks with status "success" from a prior run would be skipped.
  resetEnhanceTasks(sessionId: string) {
    const enhancedNoteIds = this.getEnhancedNoteIds(sessionId);
    const { aiTaskStore } = this.deps;
    for (const noteId of enhancedNoteIds) {
      aiTaskStore.getState().reset(createTaskId(noteId, "enhance"));
    }
  }

  enhance(sessionId: string, opts?: EnhanceOpts): EnhanceResult {
    const { aiTaskStore, getModel, getLLMConn, getSelectedTemplateId } =
      this.deps;

    const model = getModel();
    if (!model) return { type: "no_model" };

    const templateId = opts?.templateId || getSelectedTemplateId();
    const enhancedNoteId = this.ensureNote(sessionId, templateId);
    const enhanceTaskId = createTaskId(enhancedNoteId, "enhance");
    const existingTask = aiTaskStore.getState().getState(enhanceTaskId);
    if (
      existingTask?.status === "generating" ||
      existingTask?.status === "success"
    ) {
      return { type: "already_active", noteId: enhancedNoteId };
    }

    const llmConn = getLLMConn();
    void analyticsCommands.event({
      event: "note_enhanced",
      is_auto: opts?.isAuto ?? false,
      llm_provider: llmConn?.providerId,
      llm_model: llmConn?.modelId,
      template_id: templateId,
    });

    void aiTaskStore.getState().generate(enhanceTaskId, {
      model,
      taskType: "enhance",
      args: { sessionId, enhancedNoteId, templateId },
    });

    return { type: "started", noteId: enhancedNoteId };
  }

  private getTranscriptIds(sessionId: string): string[] {
    return this.deps.indexes.getSliceRowIds(
      INDEXES.transcriptBySession,
      sessionId,
    );
  }

  private getEnhancedNoteIds(sessionId: string): string[] {
    return this.deps.indexes.getSliceRowIds(
      INDEXES.enhancedNotesBySession,
      sessionId,
    );
  }

  ensureNote(sessionId: string, templateId?: string): string {
    const store = this.deps.mainStore;
    const normalizedTemplateId = templateId || undefined;

    const existingIds = this.getEnhancedNoteIds(sessionId);
    const existingId = existingIds.find((id) => {
      const tid = store.getCell("enhanced_notes", id, "template_id") as
        | string
        | undefined;
      return (tid || undefined) === normalizedTemplateId;
    });
    if (existingId) {
      if (normalizedTemplateId) {
        void this.hydrateTemplateTitle(existingId, normalizedTemplateId);
      }

      return existingId;
    }

    const enhancedNoteId = crypto.randomUUID();
    const userId = store.getValue("user_id");
    const nextPosition = existingIds.length + 1;

    store.setRow("enhanced_notes", enhancedNoteId, {
      user_id: userId || "",
      session_id: sessionId,
      content: "",
      position: nextPosition,
      title: "Summary",
      template_id: normalizedTemplateId,
    });

    if (normalizedTemplateId) {
      void this.hydrateTemplateTitle(enhancedNoteId, normalizedTemplateId);
    }

    return enhancedNoteId;
  }

  private async hydrateTemplateTitle(
    enhancedNoteId: string,
    templateId: string,
  ): Promise<void> {
    const template = await getTemplateById(templateId);
    const title = template?.title?.trim();
    if (!title) {
      return;
    }

    const currentTemplateId = this.deps.mainStore.getCell(
      "enhanced_notes",
      enhancedNoteId,
      "template_id",
    );
    if (currentTemplateId !== templateId) {
      return;
    }

    const currentTitle = this.deps.mainStore.getCell(
      "enhanced_notes",
      enhancedNoteId,
      "title",
    ) as string | undefined;
    if (!shouldHydrateTemplateTitle(currentTitle, templateId)) {
      return;
    }

    this.deps.mainStore.setCell(
      "enhanced_notes",
      enhancedNoteId,
      "title",
      title,
    );
  }
}
