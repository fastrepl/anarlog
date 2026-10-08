import {
  md2json,
  parseJsonContent,
  type JSONContent,
} from "@anlg/editor/markdown";
import {
  commands as templateCommands,
  type Participant,
  type Segment,
  type Session,
  type TemplateSection,
  type Transcript,
} from "@anlg/plugin-template";
import {
  commands as transcriptionCommands,
  type RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";
import { sessionEventSchema } from "@anlg/store";

import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";
import { collectEnhanceImageContext } from "./enhance-images";

import { resolveSummaryLanguage } from "~/services/enhancer/summary-language";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { formatSessionSourceAppsContext } from "~/session/source-apps";
import { modelSupportsImageInput } from "~/settings/ai/shared/model-capabilities";
import type { SettingValues } from "~/settings/schema";
import { parseDictionaryTermsJson } from "~/stt/keywords";
import {
  formatMeetingChatContext,
  loadMeetingChatRecords,
} from "~/stt/meeting-chat-records";
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
  const snapshot = await loadSessionContentSnapshot(sessionId, {
    includeTranscriptWords: false,
  });
  if (!snapshot) {
    throw new Error(`Session ${sessionId} no longer exists`);
  }

  const meetingChatContext = formatMeetingChatContext(
    await loadMeetingChatRecords(sessionId),
  );
  const sessionContext = getSessionContext(snapshot, meetingChatContext);
  const templateRecord = await loadTemplate(templateId);
  const memoTemplateSections =
    templateId === snapshot.rawTemplateId
      ? getMemoTemplateSections(snapshot, templateRecord?.sections ?? [])
      : null;
  let template: TaskArgsMapTransformed["enhance"]["template"] = templateRecord
    ? {
        title: templateRecord.title,
        description: templateRecord.description ?? null,
        sections: templateRecord.sections,
      }
    : null;
  if (memoTemplateSections?.length) {
    template = {
      title: templateRecord?.title ?? "Meeting memo",
      description: templateRecord?.description ?? null,
      sections: memoTemplateSections,
    };
  }
  const formatOverride = getFormatOverride(settingsValues, templateId);
  const segments = await getTranscriptSegments(snapshot);
  const transcripts = formatTranscripts(
    segments,
    sessionContext.transcriptsMeta,
  );
  const transcriptTexts = transcripts.flatMap((transcript) =>
    transcript.segments.map((segment) => segment.text),
  );
  const language = await resolveSummaryLanguage(
    settingsValues,
    transcriptTexts,
  );
  const templateSectionCount = template?.sections.length ?? 0;
  const policyResult = await templateCommands.summaryLengthPolicy({
    transcript_texts: transcriptTexts,
    template_section_count: templateSectionCount,
  });
  if (policyResult.status === "error") {
    throw new Error(policyResult.error);
  }

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
    formatOverride,
    session: sessionContext.session,
    participants: sessionContext.participants,
    template,
    preMeetingMemo: sessionContext.preMeetingMemo,
    postMeetingMemo: sessionContext.postMeetingMemo,
    transcripts,
    imageContext,
    lengthPolicy: policyResult.data,
    dictionaryTerms: parseDictionaryTermsJson(
      settingsValues.personalization_dictionary_terms,
    ),
  };
}

function getMemoTemplateSections(
  snapshot: SessionContentSnapshot,
  originalSections: TemplateSection[],
): TemplateSection[] {
  const document =
    snapshot.rawContentFormat === "markdown"
      ? md2json(snapshot.rawContent)
      : parseJsonContent(snapshot.rawContent);
  const originalByTitle = new Map(
    originalSections.map((section) => [section.title.trim(), section]),
  );
  const headings = (document.content ?? []).flatMap((node) => {
    if (node.type !== "heading" || node.attrs?.level !== 2) return [];
    const title = getNodeText(node).trim();
    return title ? [title] : [];
  });
  const preservePositionDescriptions =
    headings.length === originalSections.length;

  return headings.map((title, index) => ({
    title,
    description:
      originalByTitle.get(title)?.description ??
      (preservePositionDescriptions
        ? originalSections[index]?.description
        : null) ??
      null,
  }));
}

function getNodeText(node: JSONContent): string {
  return (
    node.text ?? node.content?.map((child) => getNodeText(child)).join("") ?? ""
  );
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
            startMs: segment.start_ms,
          }),
        ),
        startedAt: Number.isFinite(startedAt) ? startedAt : null,
        endedAt: Number.isFinite(endedAt) ? endedAt : null,
        nowMs: null,
      },
    ];
  }

  return [];
}

function getFormatOverride(
  settingsValues: SettingValues,
  templateId: string | undefined,
): string {
  if (templateId) {
    return "";
  }

  const value = settingsValues.auto_summary_prompt;
  return typeof value === "string" && value.trim() ? value : "";
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

  const meetingSourceContext = formatSessionSourceAppsContext(
    snapshot.sourceApps,
  );
  const supplementalContext = [meetingSourceContext, meetingChatContext]
    .filter((value) => value.trim())
    .join("\n\n");

  return {
    preMeetingMemo: transcriptsMeta[0]?.memoMd ?? "",
    postMeetingMemo: supplementalContext
      ? [snapshot.rawMarkdown, supplementalContext]
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
  const result = await transcriptionCommands.renderSessionTranscript({
    session_id: snapshot.sessionId,
    self_human_id: snapshot.ownerUserId || null,
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }
  if (!result.data) {
    return [];
  }

  return result.data.segments.reduce<SegmentPayload[]>((result, segment) => {
    if (segment.words.length > 0) {
      result.push(toSegmentPayload(segment));
    }
    return result;
  }, []);
}

function toSegmentPayload(segment: RenderedTranscriptSegment): SegmentPayload {
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
