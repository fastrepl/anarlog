import type {
  Participant,
  Segment,
  Session,
  Transcript,
} from "@hypr/plugin-template";
import { sessionEventSchema } from "@hypr/store";

import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";
import { collectEnhanceImageContext } from "./enhance-images";

import { loadHumansByIds } from "~/contacts/queries";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { modelSupportsImageInput } from "~/settings/ai/shared/model-capabilities";
import type { SettingValues } from "~/settings/schema";
import { getTokenAwareSummaryPrompt } from "~/shared/summary-prompt";
import {
  formatMeetingChatContext,
  loadMeetingChatRecords,
} from "~/stt/meeting-chat-records";
import {
  buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows,
  renderTranscriptSegments,
  type TranscriptRow,
} from "~/stt/render-transcript";
import { getTemplateById } from "~/templates/queries";

type TranscriptMeta = {
  id: string;
  startedAt: number;
  endedAt: number | null;
  memoMd: string;
};

type SegmentPayload = {
  speaker_label: string;
  start_ms: number;
  end_ms: number;
  text: string;
  words: Array<{ text: string; start_ms: number; end_ms: number }>;
};

export const enhanceTransform: Pick<TaskConfig<"enhance">, "transformArgs"> = {
  transformArgs,
};

async function transformArgs(
  args: TaskArgsMap["enhance"],
  settingsValues: SettingValues,
): Promise<TaskArgsMapTransformed["enhance"]> {
  const { sessionId, templateId } = args;
  const snapshot = await loadSessionContentSnapshot(sessionId);
  if (!snapshot) {
    throw new Error(`Session ${sessionId} no longer exists`);
  }

  const meetingChatContext = formatMeetingChatContext(
    await loadMeetingChatRecords(sessionId),
  );
  const sessionContext = getSessionContext(snapshot, meetingChatContext);
  const templateRecord = await loadTemplate(templateId);
  const template = templateRecord
    ? {
        title: templateRecord.title,
        description: templateRecord.description ?? null,
        sections: templateRecord.sections,
      }
    : null;
  const language = getLanguage(settingsValues);
  const customInstructions = getCustomInstructions(settingsValues);
  const segments = await getTranscriptSegments(snapshot);
  const imageContext = modelSupportsImageInput(
    getOptionalSettingsValue(settingsValues, "current_llm_provider"),
    getOptionalSettingsValue(settingsValues, "current_llm_model"),
  )
    ? await collectEnhanceImageContext(sessionId, [
        sessionContext.preMeetingMemo,
        sessionContext.postMeetingMemo,
      ])
    : [];

  return {
    language,
    customInstructions,
    session: sessionContext.session,
    participants: sessionContext.participants,
    template,
    preMeetingMemo: sessionContext.preMeetingMemo,
    postMeetingMemo: sessionContext.postMeetingMemo,
    transcripts: formatTranscripts(segments, sessionContext.transcriptsMeta),
    imageContext,
  };
}

async function loadTemplate(templateId: string | undefined) {
  if (!templateId) {
    return null;
  }

  try {
    return await getTemplateById(templateId);
  } catch (error) {
    console.error("[enhance] failed to load template", error);
    return null;
  }
}

function formatTranscripts(
  segments: SegmentPayload[],
  transcriptsMeta: TranscriptMeta[],
): Transcript[] {
  if (segments.length > 0 && transcriptsMeta.length > 0) {
    const startedAt = transcriptsMeta.reduce(
      (min, transcript) => Math.min(min, transcript.startedAt),
      Number.POSITIVE_INFINITY,
    );
    const endedAt = transcriptsMeta.reduce(
      (max, transcript) =>
        Math.max(max, transcript.endedAt ?? transcript.startedAt),
      Number.NEGATIVE_INFINITY,
    );

    return [
      {
        segments: segments.map(
          (segment): Segment => ({
            speaker: segment.speaker_label,
            text: segment.text,
          }),
        ),
        startedAt: Number.isFinite(startedAt) ? startedAt : null,
        endedAt: Number.isFinite(endedAt) ? endedAt : null,
      },
    ];
  }

  return [];
}

function getLanguage(settingsValues: SettingValues): string | null {
  const value = settingsValues.ai_language;
  return typeof value === "string" && value.length > 0 ? value : null;
}

function getCustomInstructions(settingsValues: SettingValues): string {
  const value = settingsValues.custom_summary_instructions;
  return getTokenAwareSummaryPrompt(
    typeof value === "string" ? value : "",
    settingsValues.custom_summary_instructions_token_aware === true,
  );
}

function getOptionalSettingsValue(
  settingsValues: SettingValues,
  valueId: "current_llm_provider" | "current_llm_model",
): string | undefined {
  const value = settingsValues[valueId];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function getSessionContext(
  snapshot: SessionContentSnapshot,
  meetingChatContext: string,
) {
  const transcriptsMeta = snapshot.transcripts.map((transcript) => ({
    id: transcript.id,
    startedAt: transcript.started_at,
    endedAt: transcript.ended_at,
    memoMd: transcript.memo,
  }));

  return {
    preMeetingMemo: transcriptsMeta[0]?.memoMd ?? "",
    postMeetingMemo: meetingChatContext
      ? [snapshot.rawMarkdown, meetingChatContext]
          .filter((value) => value.trim())
          .join("\n\n")
      : snapshot.rawMarkdown,
    session: getSessionData(snapshot),
    participants: getParticipants(snapshot),
    transcriptsMeta,
  };
}

function getSessionData(snapshot: SessionContentSnapshot): Session {
  const parsed = sessionEventSchema.safeParse(snapshot.event);
  if (parsed.success) {
    const eventTitle = parsed.data.title;
    return {
      title: eventTitle || snapshot.title || null,
      startedAt: parsed.data.started_at ?? null,
      endedAt: parsed.data.ended_at ?? null,
      event: {
        name: eventTitle || snapshot.title || "",
      },
    };
  }

  return {
    title: snapshot.title || null,
    startedAt: null,
    endedAt: null,
    event: null,
  };
}

function getParticipants(snapshot: SessionContentSnapshot): Participant[] {
  return snapshot.participants
    .filter((participant) => participant.name)
    .map((participant) => ({
      name: participant.name,
      jobTitle: participant.jobTitle || null,
    }));
}

async function getTranscriptSegments(
  snapshot: SessionContentSnapshot,
): Promise<SegmentPayload[]> {
  if (snapshot.transcripts.length === 0) {
    return [];
  }

  const transcriptRows: TranscriptRow[] = snapshot.transcripts.map(
    (transcript) => ({
      started_at: transcript.started_at,
      words: transcript.words,
      speaker_hints: transcript.speaker_hints,
    }),
  );
  const humanIds = [
    snapshot.ownerUserId,
    ...snapshot.participants.map((participant) => participant.humanId),
    ...collectAssignedHumanIdsFromTranscriptRows(transcriptRows),
  ];
  const humans = await loadHumansByIds(humanIds);
  const request = buildRenderTranscriptRequestFromRows(
    transcriptRows,
    {
      selfHumanId: snapshot.ownerUserId || undefined,
      humans: humans
        .filter((human) => human.name)
        .map((human) => ({ human_id: human.id, name: human.name })),
    },
    snapshot.participants.map((participant) => participant.humanId),
  );
  if (!request) {
    return [];
  }

  const segments = await renderTranscriptSegments(request);

  return segments
    .reduce<SegmentPayload[]>((result, segment) => {
      if (segment.words.length > 0) {
        result.push(toSegmentPayload(segment));
      }
      return result;
    }, [])
    .sort((left, right) => left.start_ms - right.start_ms);
}

function toSegmentPayload(
  segment: Awaited<ReturnType<typeof renderTranscriptSegments>>[number],
): SegmentPayload {
  return {
    speaker_label: segment.speaker_label,
    start_ms: segment.start_ms,
    end_ms: segment.end_ms,
    text: segment.text,
    words: segment.words.map((word) => ({
      text: word.text,
      start_ms: word.start_ms,
      end_ms: word.end_ms,
    })),
  };
}
