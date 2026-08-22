import { t } from "@lingui/core/macro";
import { useCallback, useRef } from "react";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  runPreMeetingBriefJob,
  usePreMeetingBriefGenerating,
  useRegisterPreMeetingBriefEditor,
  type MemoBriefEditor,
} from "./pre-meeting-brief-job";

import { useLanguageModel } from "~/ai/hooks";
import { useNow } from "~/calendar/hooks";
import { useSessionCalendarEvent } from "~/calendar/queries";
import { getStoredNoteMarkdown } from "~/session/components/note-input/header-shared";
import { hasStoredNoteContent } from "~/session/components/shared";
import { usePastSessionNotes } from "~/session/insights/past-notes";
import {
  canCreatePreMeetingBrief,
  selectBriefSourceNotes,
  shouldShowPreMeetingBrief,
} from "~/session/insights/pre-meeting";
import { useSession, useSessionParticipants } from "~/session/queries";
import { useConfigValue } from "~/shared/config";
import type { SessionMode } from "~/store/zustand/listener/general";

export type { MemoBriefEditor };

export function useCreatePreMeetingBrief({
  sessionId,
  sessionMode,
  isMemoEmpty,
  onSwitchToMemos,
  getMemoEditor,
}: {
  sessionId: string;
  sessionMode: SessionMode;
  isMemoView: boolean;
  isMemoEmpty?: boolean;
  onSwitchToMemos: () => void;
  getMemoEditor: () => MemoBriefEditor | null;
}) {
  const now = useNow();
  const language = useConfigValue("ai_language") || "en";
  const model = useLanguageModel("enhance");
  const enabled = sessionMode === "inactive";
  const event = useSessionCalendarEvent(sessionId, { enabled });
  const upcoming = shouldShowPreMeetingBrief(event, now.getTime());
  const session = useSession(sessionId);
  const participants = useSessionParticipants(sessionId);
  const hasParticipants = participants.some(
    (participant) =>
      participant.source !== "excluded" &&
      Boolean(participant.humanId) &&
      participant.humanId !== session?.user_id,
  );
  const pastNotes = usePastSessionNotes(sessionId, {
    enabled: enabled && (upcoming || hasParticipants),
  });
  const memoEmpty = isMemoEmpty ?? !hasStoredNoteContent(session?.raw_md);
  const available =
    enabled &&
    Boolean(model) &&
    memoEmpty &&
    canCreatePreMeetingBrief({
      event,
      nowMs: now.getTime(),
      notes: pastNotes.notes,
      hasParticipants,
    });
  const isGenerating = usePreMeetingBriefGenerating(sessionId);
  useRegisterPreMeetingBriefEditor(sessionId, getMemoEditor);

  const eventRef = useRef(event);
  eventRef.current = event;
  const notesRef = useRef(pastNotes.notes);
  notesRef.current = pastNotes.notes;
  const sessionRef = useRef(session);
  sessionRef.current = session;
  const participantsRef = useRef(participants);
  participantsRef.current = participants;

  const createBrief = useCallback(() => {
    if (!available || isGenerating || !model) {
      return;
    }

    const notes = selectBriefSourceNotes(notesRef.current);
    if (notes.length === 0) {
      return;
    }

    const briefEvent = eventRef.current ?? {
      title: sessionRef.current?.title,
      participants: participantsRef.current
        .filter(
          (participant) =>
            participant.source !== "excluded" &&
            participant.humanId !== sessionRef.current?.user_id,
        )
        .map((participant) => ({
          name: participant.name,
          email: participant.email,
        })),
    };

    onSwitchToMemos();
    void runPreMeetingBriefJob({
      sessionId,
      model,
      language,
      event: briefEvent,
      notes,
      existingMarkdown: memoEmpty
        ? ""
        : getStoredNoteMarkdown(sessionRef.current?.raw_md),
    }).catch((error) => {
      console.error("Failed to create pre-meeting brief", error);
      sonnerToast.error(t`Could not create the pre-meeting brief. Try again.`, {
        id: "pre-meeting-brief-error",
      });
    });
  }, [
    available,
    isGenerating,
    language,
    memoEmpty,
    model,
    onSwitchToMemos,
    sessionId,
  ]);

  return {
    visible: available || isGenerating,
    isGenerating,
    createBrief,
  };
}
