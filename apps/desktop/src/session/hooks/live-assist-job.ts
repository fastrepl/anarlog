import type { LanguageModel } from "ai";
import { useSyncExternalStore } from "react";

import {
  streamLiveAssistSuggestion,
  type LiveAssistKind,
} from "~/session/insights/live-assist";
import { useLiveAssistStore } from "~/store/zustand/live-assist";

export type LiveAssistRunContext = {
  model: LanguageModel;
  language: string;
  sourceText: string;
};

// A single kind can only appear once per session's pending queue: firing it
// twice before the first run even starts would just waste a model call.
const queueBySession = new Map<string, LiveAssistKind[]>();
const runningSessions = new Set<string>();
const abortControllersBySession = new Map<string, AbortController>();
// Built at run time (not enqueue time) so the source text reflects the
// freshest transcript window available once it's this kind's turn to run.
const contextBuildersBySession = new Map<
  string,
  (kind: LiveAssistKind) => Promise<LiveAssistRunContext | null>
>();
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) {
    listener();
  }
}

function subscribeLiveAssistQueue(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function registerLiveAssistContextBuilder(
  sessionId: string,
  builder: (kind: LiveAssistKind) => Promise<LiveAssistRunContext | null>,
) {
  contextBuildersBySession.set(sessionId, builder);
  return () => {
    if (contextBuildersBySession.get(sessionId) === builder) {
      contextBuildersBySession.delete(sessionId);
    }
  };
}

export function enqueueLiveAssistKind(sessionId: string, kind: LiveAssistKind) {
  const queue = queueBySession.get(sessionId) ?? [];
  if (queue.includes(kind)) {
    return;
  }
  queue.push(kind);
  queueBySession.set(sessionId, queue);
  emit();
  void runLiveAssistWorker(sessionId);
}

export function useLiveAssistQueueLength(sessionId: string): number {
  return useSyncExternalStore(
    subscribeLiveAssistQueue,
    () => queueBySession.get(sessionId)?.length ?? 0,
    () => 0,
  );
}

// Aborts anything in flight and drops pending kinds for a session; existing
// "ready"/"error" cards in the Zustand store are left alone on purpose so a
// paused or stopped session still shows what was already generated. Callers
// that also want the cards gone (true end of session) clear the store too.
export function resetLiveAssistJobs(sessionId: string) {
  queueBySession.delete(sessionId);
  contextBuildersBySession.delete(sessionId);
  abortControllersBySession.get(sessionId)?.abort();
  abortControllersBySession.delete(sessionId);
  emit();
}

async function runLiveAssistWorker(sessionId: string) {
  if (runningSessions.has(sessionId)) {
    return;
  }
  runningSessions.add(sessionId);
  emit();

  try {
    for (;;) {
      const queue = queueBySession.get(sessionId);
      const kind = queue?.shift();
      if (!kind || !queue) {
        break;
      }
      if (queue.length === 0) {
        queueBySession.delete(sessionId);
      }
      emit();
      await runLiveAssistKind(sessionId, kind);
    }
  } finally {
    runningSessions.delete(sessionId);
    emit();
  }
}

async function runLiveAssistKind(sessionId: string, kind: LiveAssistKind) {
  const buildContext = contextBuildersBySession.get(sessionId);
  if (!buildContext) {
    return;
  }

  let context: LiveAssistRunContext | null;
  try {
    context = await buildContext(kind);
  } catch (error) {
    console.error("Failed to build Live Assist context", kind, error);
    return;
  }
  if (!context || !context.sourceText.trim()) {
    return;
  }

  const cardId = `${sessionId}:${kind}:${Date.now()}`;
  useLiveAssistStore.getState().addGeneratingCard(sessionId, {
    id: cardId,
    kind,
    createdAtMs: Date.now(),
  });

  const controller = new AbortController();
  abortControllersBySession.set(sessionId, controller);

  try {
    const items = await streamLiveAssistSuggestion({
      model: context.model,
      language: context.language,
      kind,
      sourceText: context.sourceText,
      signal: controller.signal,
    });
    if (items.length === 0) {
      throw new Error("Live Assist returned no items");
    }
    useLiveAssistStore
      .getState()
      .resolveCard(sessionId, cardId, { status: "ready", items });
  } catch (error) {
    if (controller.signal.aborted) {
      useLiveAssistStore.getState().removeCard(sessionId, cardId);
      return;
    }
    console.error("Failed to generate Live Assist suggestion", kind, error);
    useLiveAssistStore.getState().resolveCard(sessionId, cardId, {
      status: "error",
      errorMessage: error instanceof Error ? error.message : String(error),
    });
  } finally {
    if (abortControllersBySession.get(sessionId) === controller) {
      abortControllersBySession.delete(sessionId);
    }
  }
}
