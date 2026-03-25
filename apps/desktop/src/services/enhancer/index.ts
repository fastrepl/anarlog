import { generateText, type LanguageModel } from "ai";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { json2md, parseJsonContent } from "@hypr/tiptap/shared";

import { getEligibility } from "./eligibility";

import type { Store as MainStore } from "~/store/tinybase/store/main";
import { INDEXES } from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import type { TasksActions } from "~/store/zustand/ai-task/tasks";
import { listenerStore } from "~/store/zustand/listener/instance";

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
  private activeContactSummary = new Set<string>();
  private queuedContactSummary = new Set<string>();
  private pendingRetries = new Map<string, ReturnType<typeof setTimeout>>();
  private unsubscribe: (() => void) | null = null;
  private eventListeners = new Set<(event: EnhancerEvent) => void>();
  private storeListenerIds: string[] = [];

  constructor(private deps: EnhancerDeps) {}

  start() {
    this.unsubscribe = listenerStore.subscribe((state) => {
      const { status, sessionId } = state.live;

      if (status === "active" && sessionId) {
        this.activeAutoEnhance.delete(sessionId);
        this.clearRetry(sessionId);
      }
    });

    const { mainStore } = this.deps;

    this.storeListenerIds = [
      mainStore.addRowListener(
        "sessions",
        null,
        (_store, _tableId, rowId, getCellChange) => {
          if (
            !getCellChange ||
            (!getCellChange("sessions", rowId, "raw_md")[0] &&
              !getCellChange("sessions", rowId, "title")[0] &&
              !getCellChange("sessions", rowId, "event_json")[0])
          ) {
            return;
          }

          this.queueContactSummariesForSession(rowId);
        },
      ),
      mainStore.addRowListener(
        "enhanced_notes",
        null,
        (store, _tableId, rowId, getCellChange) => {
          if (!getCellChange) {
            return;
          }

          const [contentChanged] = getCellChange(
            "enhanced_notes",
            rowId,
            "content",
          );
          const [sessionChanged, previousSessionId, nextSessionId] =
            getCellChange("enhanced_notes", rowId, "session_id");

          if (!contentChanged && !sessionChanged) {
            return;
          }

          if (typeof previousSessionId === "string" && previousSessionId) {
            this.queueContactSummariesForSession(previousSessionId);
          }
          if (typeof nextSessionId === "string" && nextSessionId) {
            this.queueContactSummariesForSession(nextSessionId);
            return;
          }

          const sessionId = store.getCell(
            "enhanced_notes",
            rowId,
            "session_id",
          );
          if (typeof sessionId === "string" && sessionId) {
            this.queueContactSummariesForSession(sessionId);
          }
        },
      ),
      mainStore.addRowListener(
        "mapping_session_participant",
        null,
        (_store, _tableId, rowId, getCellChange) => {
          if (!getCellChange) {
            return;
          }

          const candidateHumanIds = new Set<string>();
          const [, previousHumanId, nextHumanId] = getCellChange(
            "mapping_session_participant",
            rowId,
            "human_id",
          );

          if (typeof previousHumanId === "string" && previousHumanId) {
            candidateHumanIds.add(previousHumanId);
          }
          if (typeof nextHumanId === "string" && nextHumanId) {
            candidateHumanIds.add(nextHumanId);
          }

          for (const humanId of candidateHumanIds) {
            this.queueContactSummaryUpdate(humanId);
          }
        },
      ),
    ];
  }

  dispose() {
    this.unsubscribe?.();
    this.unsubscribe = null;
    for (const timer of this.pendingRetries.values()) clearTimeout(timer);
    this.pendingRetries.clear();
    this.activeAutoEnhance.clear();
    this.activeContactSummary.clear();
    this.queuedContactSummary.clear();
    this.eventListeners.clear();
    for (const listenerId of this.storeListenerIds) {
      this.deps.mainStore.delListener(listenerId);
    }
    this.storeListenerIds = [];
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
    if (existingId) return existingId;

    const enhancedNoteId = crypto.randomUUID();
    const userId = store.getValue("user_id");
    const nextPosition = existingIds.length + 1;

    let title = "Summary";
    if (normalizedTemplateId) {
      const templateTitle = store.getCell(
        "templates",
        normalizedTemplateId,
        "title",
      );
      if (typeof templateTitle === "string") title = templateTitle;
    }

    store.setRow("enhanced_notes", enhancedNoteId, {
      user_id: userId || "",
      session_id: sessionId,
      content: "",
      position: nextPosition,
      title,
      template_id: normalizedTemplateId,
    });

    return enhancedNoteId;
  }

  queueContactSummariesForSession(sessionId: string) {
    for (const humanId of this.getParticipantHumanIds(sessionId)) {
      this.queueContactSummaryUpdate(humanId);
    }
  }

  queueContactSummaryUpdate(humanId: string) {
    if (!humanId) {
      return;
    }

    if (this.activeContactSummary.has(humanId)) {
      this.queuedContactSummary.add(humanId);
      return;
    }

    this.activeContactSummary.add(humanId);

    void this.generateContactSummary(humanId).finally(() => {
      this.activeContactSummary.delete(humanId);

      if (this.queuedContactSummary.delete(humanId)) {
        this.queueContactSummaryUpdate(humanId);
      }
    });
  }

  private async generateContactSummary(humanId: string) {
    const model = this.deps.getModel();
    if (!model) {
      return;
    }

    const store = this.deps.mainStore;
    if (!store.getRow("humans", humanId)) {
      return;
    }

    const summaryInput = this.buildContactSummaryInput(humanId);

    if (!summaryInput) {
      store.setPartialRow("humans", humanId, { summary: "" });
      return;
    }

    try {
      const result = await generateText({
        model,
        temperature: 0,
        system: CONTACT_SUMMARY_SYSTEM_PROMPT,
        prompt: buildContactSummaryPrompt(summaryInput),
      });

      store.setPartialRow("humans", humanId, {
        summary: result.text.trim(),
      });
    } catch (error) {
      console.error("Failed to generate contact summary:", error);
    }
  }

  private buildContactSummaryInput(humanId: string) {
    const store = this.deps.mainStore;
    const human = store.getRow("humans", humanId);
    if (!human) {
      return null;
    }

    const sessionIds = this.getSessionIdsForHuman(humanId);
    const sessions = sessionIds
      .map((sessionId) => this.getSessionSummarySource(sessionId))
      .filter(
        (session): session is NonNullable<typeof session> => session !== null,
      )
      .slice(0, CONTACT_SUMMARY_SESSION_LIMIT);

    const memo =
      typeof human.memo === "string" && human.memo.trim()
        ? human.memo.trim()
        : "";
    if (!memo && sessions.length === 0) {
      return null;
    }

    return {
      name:
        (typeof human.name === "string" && human.name.trim()) ||
        (typeof human.email === "string" && human.email.trim()) ||
        "Unknown contact",
      memo,
      sessions,
    };
  }

  private getSessionIdsForHuman(humanId: string): string[] {
    const mappingIds = this.deps.indexes.getSliceRowIds(
      INDEXES.sessionsByHuman,
      humanId,
    );
    const sessionIds = new Set<string>();

    for (const mappingId of mappingIds) {
      const source = this.deps.mainStore.getCell(
        "mapping_session_participant",
        mappingId,
        "source",
      );
      if (source === "excluded") {
        continue;
      }

      const sessionId = this.deps.mainStore.getCell(
        "mapping_session_participant",
        mappingId,
        "session_id",
      );
      if (typeof sessionId === "string" && sessionId) {
        sessionIds.add(sessionId);
      }
    }

    return Array.from(sessionIds).sort((a, b) => {
      return this.getSessionSortKey(b) - this.getSessionSortKey(a);
    });
  }

  private getParticipantHumanIds(sessionId: string): string[] {
    const mappingIds = this.deps.indexes.getSliceRowIds(
      INDEXES.sessionParticipantsBySession,
      sessionId,
    );
    const humanIds = new Set<string>();

    for (const mappingId of mappingIds) {
      const source = this.deps.mainStore.getCell(
        "mapping_session_participant",
        mappingId,
        "source",
      );
      if (source === "excluded") {
        continue;
      }

      const humanId = this.deps.mainStore.getCell(
        "mapping_session_participant",
        mappingId,
        "human_id",
      );
      if (typeof humanId === "string" && humanId) {
        humanIds.add(humanId);
      }
    }

    return Array.from(humanIds);
  }

  private getSessionSummarySource(sessionId: string) {
    const session = this.deps.mainStore.getRow("sessions", sessionId);
    if (!session) {
      return null;
    }

    const sections: string[] = [];
    const enhancedNotes = this.getEnhancedNoteIds(sessionId);
    for (const noteId of enhancedNotes) {
      const markdown = jsonToMarkdown(
        this.deps.mainStore.getCell("enhanced_notes", noteId, "content"),
      );
      if (markdown) {
        sections.push(`AI summary\n${truncatePromptChunk(markdown)}`);
      }
    }

    const rawNotes = jsonToMarkdown(session.raw_md);
    if (rawNotes) {
      sections.push(`Manual notes\n${truncatePromptChunk(rawNotes)}`);
    }

    if (sections.length === 0) {
      return null;
    }

    return {
      title:
        (typeof session.title === "string" && session.title.trim()) ||
        "Untitled note",
      happenedAt: this.getSessionDateLabel(sessionId),
      content: sections.join("\n\n"),
    };
  }

  private getSessionDateLabel(sessionId: string): string {
    const eventJson = this.deps.mainStore.getCell(
      "sessions",
      sessionId,
      "event_json",
    );
    if (typeof eventJson === "string" && eventJson.trim()) {
      try {
        const parsed = JSON.parse(eventJson) as { started_at?: string };
        if (typeof parsed.started_at === "string" && parsed.started_at) {
          return parsed.started_at;
        }
      } catch {
        // Ignore invalid event JSON and fall back to created_at.
      }
    }

    const createdAt = this.deps.mainStore.getCell(
      "sessions",
      sessionId,
      "created_at",
    );
    return typeof createdAt === "string" ? createdAt : "";
  }

  private getSessionSortKey(sessionId: string): number {
    const label = this.getSessionDateLabel(sessionId);
    const parsed = Date.parse(label);
    return Number.isNaN(parsed) ? 0 : parsed;
  }
}

const CONTACT_SUMMARY_SESSION_LIMIT = 12;
const CONTACT_SUMMARY_CHUNK_LIMIT = 1500;

const CONTACT_SUMMARY_SYSTEM_PROMPT = `You write concise relationship summaries for a contacts view in a meeting notes app.

Use only the provided notes.
Keep the output under 140 words.
Focus on the person's role, active workstreams, commitments, preferences, and follow-ups worth remembering.
Prefer concrete facts over generic framing.
Do not mention missing information or speculate.
Return plain text only.`;

function buildContactSummaryPrompt(input: {
  name: string;
  memo: string;
  sessions: Array<{ title: string; happenedAt: string; content: string }>;
}) {
  const parts = [`Contact: ${input.name}`];

  if (input.memo) {
    parts.push(`Manual contact notes:\n${truncatePromptChunk(input.memo)}`);
  }

  const sessionBlocks = input.sessions.map((session, index) => {
    const heading = [`Meeting ${index + 1}: ${session.title}`];
    if (session.happenedAt) {
      heading.push(`Date: ${session.happenedAt}`);
    }

    return `${heading.join("\n")}\n${session.content}`;
  });

  parts.push(`Meetings:\n${sessionBlocks.join("\n\n---\n\n")}`);

  return `${parts.join("\n\n")}\n\nWrite one concise relationship summary for this contact.`;
}

function truncatePromptChunk(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length <= CONTACT_SUMMARY_CHUNK_LIMIT) {
    return trimmed;
  }

  return `${trimmed.slice(0, CONTACT_SUMMARY_CHUNK_LIMIT).trimEnd()}...`;
}

function jsonToMarkdown(value: unknown): string {
  if (typeof value !== "string" || !value.trim()) {
    return "";
  }

  const trimmed = value.trim();
  if (!trimmed.startsWith("{")) {
    return trimmed;
  }

  return json2md(parseJsonContent(trimmed)).trim();
}
