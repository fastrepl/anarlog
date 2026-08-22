import { t } from "@lingui/core/macro";
import { useMutation } from "@tanstack/react-query";
import { useCallback, useRef } from "react";

import { md2json, type JSONContent } from "@anlg/editor/markdown";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useLanguageModel } from "~/ai/hooks";
import { useNow } from "~/calendar/hooks";
import { useSessionCalendarEvent } from "~/calendar/queries";
import { getStoredNoteMarkdown } from "~/session/components/note-input/header-shared";
import { usePastSessionNotes } from "~/session/insights/past-notes";
import {
  canCreatePreMeetingBrief,
  mergeBriefMarkdown,
  selectBriefSourceNotes,
  shouldShowPreMeetingBrief,
  streamPreMeetingBrief,
} from "~/session/insights/pre-meeting";
import { updateSession, useSession } from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import type { SessionMode } from "~/store/zustand/listener/general";

export type MemoBriefEditor = {
  replaceContent: (content: JSONContent) => void;
  flushPendingChanges: () => void;
};

export function useCreatePreMeetingBrief({
  sessionId,
  sessionMode,
  isMemoView,
  onSwitchToMemos,
  getMemoEditor,
}: {
  sessionId: string;
  sessionMode: SessionMode;
  isMemoView: boolean;
  onSwitchToMemos: () => void;
  getMemoEditor: () => MemoBriefEditor | null;
}) {
  const now = useNow();
  const language = useConfigValue("ai_language") || "en";
  const model = useLanguageModel("enhance");
  const enabled = sessionMode === "inactive";
  const event = useSessionCalendarEvent(sessionId, { enabled });
  const upcoming = shouldShowPreMeetingBrief(event, now.getTime());
  const pastNotes = usePastSessionNotes(sessionId, {
    enabled: enabled && upcoming,
  });
  const session = useSession(sessionId);
  const visible =
    enabled &&
    Boolean(model) &&
    canCreatePreMeetingBrief({
      event,
      nowMs: now.getTime(),
      notes: pastNotes.notes,
    });

  const isMemoViewRef = useRef(isMemoView);
  isMemoViewRef.current = isMemoView;
  const getMemoEditorRef = useRef(getMemoEditor);
  getMemoEditorRef.current = getMemoEditor;
  const eventRef = useRef(event);
  eventRef.current = event;
  const notesRef = useRef(pastNotes.notes);
  notesRef.current = pastNotes.notes;
  const sessionRef = useRef(session);
  sessionRef.current = session;

  const { mutate, isPending } = useMutation({
    mutationKey: ["pre-meeting-brief", sessionId],
    mutationFn: async () => {
      if (!model || !eventRef.current) {
        return;
      }

      const notes = selectBriefSourceNotes(notesRef.current);
      if (notes.length === 0) {
        return;
      }

      onSwitchToMemos();
      const existingMarkdown = getStoredNoteMarkdown(
        sessionRef.current?.raw_md,
      );
      let latest = "";
      const brief = await streamPreMeetingBrief({
        model,
        language,
        event: eventRef.current,
        notes,
        onText: (text) => {
          latest = text;
          if (!existingMarkdown) {
            applyBriefMarkdown(latest, getMemoEditorRef, isMemoViewRef);
          }
        },
      });
      if (!brief) {
        throw new Error("empty-brief");
      }

      const markdown = mergeBriefMarkdown(brief, existingMarkdown);
      const editor = await waitForMemoEditor(getMemoEditorRef, isMemoViewRef);
      if (editor) {
        editor.replaceContent(md2json(markdown));
        editor.flushPendingChanges();
        return;
      }

      await updateSession(sessionId, {
        raw_md: JSON.stringify(md2json(markdown)),
      });
    },
    onError: (error) => {
      console.error("Failed to create pre-meeting brief", error);
      sonnerToast.error(t`Could not create the pre-meeting brief. Try again.`, {
        id: "pre-meeting-brief-error",
      });
    },
  });

  const createBrief = useCallback(() => {
    if (!visible || isPending) {
      return;
    }

    mutate();
  }, [isPending, mutate, visible]);

  return {
    visible,
    isGenerating: isPending,
    createBrief,
  };
}

function applyBriefMarkdown(
  markdown: string,
  getMemoEditorRef: { current: () => MemoBriefEditor | null },
  isMemoViewRef: { current: boolean },
) {
  if (!isMemoViewRef.current) {
    return;
  }

  const editor = getMemoEditorRef.current();
  if (!editor || !markdown.trim()) {
    return;
  }

  editor.replaceContent(md2json(markdown));
}

async function waitForMemoEditor(
  getMemoEditorRef: { current: () => MemoBriefEditor | null },
  isMemoViewRef: { current: boolean },
): Promise<MemoBriefEditor | null> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (isMemoViewRef.current) {
      const editor = getMemoEditorRef.current();
      if (editor) {
        return editor;
      }
    }

    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => resolve());
    });
  }

  return isMemoViewRef.current ? getMemoEditorRef.current() : null;
}
