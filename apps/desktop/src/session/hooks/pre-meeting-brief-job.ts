import type { LanguageModel } from "ai";
import { useEffect, useSyncExternalStore } from "react";

import { md2json, type JSONContent } from "@anlg/editor/markdown";

import type { PastSessionNote } from "~/session/insights/past-notes";
import {
  mergeBriefMarkdown,
  streamPreMeetingBrief,
  type PreMeetingBriefEvent,
} from "~/session/insights/pre-meeting";
import { updateSession } from "~/session/queries";

export type MemoBriefEditor = {
  replaceContent: (content: JSONContent) => void;
  flushPendingChanges: () => void;
};

const generating = new Set<string>();
const listeners = new Set<() => void>();
const editors = new Map<string, () => MemoBriefEditor | null>();

function emit() {
  for (const listener of listeners) {
    listener();
  }
}

export function subscribePreMeetingBriefJobs(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function isPreMeetingBriefGenerating(sessionId: string) {
  return generating.has(sessionId);
}

export function registerPreMeetingBriefEditor(
  sessionId: string,
  getEditor: () => MemoBriefEditor | null,
) {
  editors.set(sessionId, getEditor);
  return () => {
    if (editors.get(sessionId) === getEditor) {
      editors.delete(sessionId);
    }
  };
}

export function usePreMeetingBriefGenerating(sessionId: string) {
  return useSyncExternalStore(
    subscribePreMeetingBriefJobs,
    () => isPreMeetingBriefGenerating(sessionId),
    () => false,
  );
}

export function useRegisterPreMeetingBriefEditor(
  sessionId: string,
  getEditor: () => MemoBriefEditor | null,
) {
  useEffect(() => {
    return registerPreMeetingBriefEditor(sessionId, getEditor);
  }, [getEditor, sessionId]);
}

export function resetPreMeetingBriefJobs() {
  generating.clear();
  editors.clear();
  emit();
}

function applyBriefToEditor(
  sessionId: string,
  markdown: string,
  flush: boolean,
) {
  const editor = editors.get(sessionId)?.();
  if (!editor || !markdown.trim()) {
    return false;
  }

  editor.replaceContent(md2json(markdown));
  if (flush) {
    editor.flushPendingChanges();
  }
  return true;
}

function persistBrief(sessionId: string, markdown: string) {
  return updateSession(sessionId, {
    raw_md: JSON.stringify(md2json(markdown)),
  });
}

export async function runPreMeetingBriefJob({
  sessionId,
  model,
  language,
  event,
  notes,
  existingMarkdown,
}: {
  sessionId: string;
  model: LanguageModel;
  language: string;
  event: PreMeetingBriefEvent;
  notes: PastSessionNote[];
  existingMarkdown: string;
}) {
  if (generating.has(sessionId)) {
    return;
  }

  generating.add(sessionId);
  emit();

  try {
    const brief = await streamPreMeetingBrief({
      model,
      language,
      event,
      notes,
      onText: (text) => {
        const markdown = mergeBriefMarkdown(text, existingMarkdown);
        if (!applyBriefToEditor(sessionId, markdown, false)) {
          void persistBrief(sessionId, markdown);
        }
      },
    });
    if (!brief) {
      throw new Error("empty-brief");
    }

    const markdown = mergeBriefMarkdown(brief, existingMarkdown);
    if (!applyBriefToEditor(sessionId, markdown, true)) {
      await persistBrief(sessionId, markdown);
    }
  } finally {
    generating.delete(sessionId);
    emit();
  }
}
