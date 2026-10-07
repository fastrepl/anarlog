import levenshtein from "js-levenshtein-esm";

import { md2json, parseJsonContent } from "@anlg/editor/markdown";
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
import { normalizeSummaryLengthMode } from "~/services/enhancer/summary-length";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import type { AppliedTemplateSnapshot } from "~/session/queries/types";
import { formatSessionSourceAppsContext } from "~/session/source-apps";
import { extractSectionHeadings } from "~/session/title-content";
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
  const preferTemplate = isTemplateNewerThanMemo(
    templateRecord?.updatedAt,
    snapshot.rawHeadingsUpdatedAt || snapshot.rawUpdatedAt,
  );
  const appliedSnapshot =
    snapshot.rawAppliedTemplate?.templateId === templateId
      ? snapshot.rawAppliedTemplate
      : null;
  const memoTemplateSections =
    templateId === snapshot.rawTemplateId
      ? getMemoTemplateSections(snapshot, templateRecord?.sections ?? [], {
          preferTemplate,
          appliedSnapshot,
          templateMissing: !templateRecord,
        })
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
  const summaryLength = normalizeSummaryLengthMode(
    settingsValues.summary_length,
  );
  const templateSectionCount = template?.sections.length ?? 0;
  const policyResult = await templateCommands.summaryLengthPolicy({
    transcript_texts: transcriptTexts,
    mode: summaryLength,
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
    summaryLength,
    lengthPolicy: policyResult.data,
    dictionaryTerms: parseDictionaryTermsJson(
      settingsValues.personalization_dictionary_terms,
    ),
  };
}

function isTemplateNewerThanMemo(
  templateUpdatedAt: string | undefined,
  memoUpdatedAt: string,
): boolean {
  if (!templateUpdatedAt || !memoUpdatedAt) {
    return false;
  }
  const templateTime = Date.parse(templateUpdatedAt);
  const memoTime = Date.parse(memoUpdatedAt);
  if (Number.isNaN(templateTime) || Number.isNaN(memoTime)) {
    return false;
  }
  return templateTime > memoTime;
}

function normalizeHeadingTitle(title: string): string {
  return title.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
}

// Similarity is a hint, not proof, of a rename. Limits: a residue candidate
// must share its characters in order with the live title (spacing,
// punctuation, and affix edits only, no word substitutions), within a
// stricter 25% relative threshold than enhance-validator's 30% acceptance
// rule. Word substitutions ("Hiring"/"Firing") and dissimilar renames or
// removals are intentionally kept as memo-only sections because without a
// snapshot they are indistinguishable from genuine user additions; same-root
// affix collisions ("Actions"/"Auctions") can still drop and are an accepted
// residual. Callers must only pass unmatched live titles: a memo heading can
// only be residue of a rename the template actually made. This check runs
// only when the template is authoritative (preferTemplate), never to
// overrule a memo-side rename.
function isSubsequence(shorter: string, longer: string): boolean {
  let index = 0;
  for (const char of longer) {
    if (char === shorter[index]) {
      index++;
    }
  }
  return index === shorter.length;
}

function looksLikeRenameOf(title: string, templateTitles: string[]): boolean {
  const normalized = normalizeHeadingTitle(title);
  if (!normalized) {
    return false;
  }
  return templateTitles.some((templateTitle) => {
    const candidate = normalizeHeadingTitle(templateTitle);
    if (!candidate) {
      return false;
    }
    const threshold = Math.floor(
      Math.min(normalized.length, candidate.length) * 0.25,
    );
    const [shorter, longer] =
      normalized.length <= candidate.length
        ? [normalized, candidate]
        : [candidate, normalized];
    return (
      levenshtein(normalized, candidate) <= threshold &&
      isSubsequence(shorter, longer)
    );
  });
}

type HeadingGap = {
  memo: Array<{ title: string; index: number }>;
  base: string[];
};

// Splits memo headings and snapshot titles into gaps between their
// longest-common-subsequence anchors. Headings both sides share (even in a
// different order) anchor the alignment, so insertions and moves do not
// shift the pairing of what changed between them.
function alignHeadingGaps(memo: string[], base: string[]): HeadingGap[] {
  const lengths: number[][] = Array.from({ length: memo.length + 1 }, () =>
    new Array<number>(base.length + 1).fill(0),
  );
  for (let i = memo.length - 1; i >= 0; i--) {
    for (let j = base.length - 1; j >= 0; j--) {
      lengths[i][j] =
        memo[i] === base[j]
          ? lengths[i + 1][j + 1] + 1
          : Math.max(lengths[i + 1][j], lengths[i][j + 1]);
    }
  }
  const anchors: Array<{ memo: number; base: number }> = [];
  let i = 0;
  let j = 0;
  while (i < memo.length && j < base.length) {
    if (memo[i] === base[j]) {
      anchors.push({ memo: i, base: j });
      i++;
      j++;
    } else if (lengths[i + 1][j] >= lengths[i][j + 1]) {
      i++;
    } else {
      j++;
    }
  }
  const gaps: HeadingGap[] = [];
  let prevMemo = -1;
  let prevBase = -1;
  const pushGap = (nextMemo: number, nextBase: number) => {
    const memoGap = memo
      .slice(prevMemo + 1, nextMemo)
      .map((title, offset) => ({ title, index: prevMemo + 1 + offset }));
    const baseGap = base.slice(prevBase + 1, nextBase);
    if (memoGap.length > 0 || baseGap.length > 0) {
      gaps.push({ memo: memoGap, base: baseGap });
    }
    prevMemo = nextMemo;
    prevBase = nextBase;
  };
  for (const anchor of anchors) {
    pushGap(anchor.memo, anchor.base);
  }
  pushGap(memo.length, base.length);
  return gaps;
}

function mergeWithAppliedSnapshot(
  headings: string[],
  snapshotTitles: string[],
  originalSections: TemplateSection[],
): TemplateSection[] {
  const snapshotSet = new Set(snapshotTitles);
  const templateByTitle = new Map(
    originalSections.map((section) => [section.title.trim(), section]),
  );
  const memoSet = new Set(headings);

  if (
    headings.length === originalSections.length &&
    headings.every((title) => templateByTitle.has(title)) &&
    originalSections.every((section) => memoSet.has(section.title.trim()))
  ) {
    const memoMatchesSnapshot =
      headings.length === snapshotTitles.length &&
      headings.every((title, index) => title === snapshotTitles[index]);
    if (memoMatchesSnapshot) {
      return originalSections.map((section) => ({
        title: section.title,
        description: section.description ?? "",
      }));
    }
    return headings.map((title) => ({
      title,
      description: templateByTitle.get(title)?.description ?? "",
    }));
  }

  // Pair memo-only headings with base-only headings inside each aligned
  // gap: equal-length gaps are positional renames, so a memo insertion
  // before a renamed heading no longer shifts the pairing. The base title
  // must survive in the live template; when the template changed it too,
  // both headings stand alone instead of guessing.
  const renameTarget = new Map<string, number>();
  for (const gap of alignHeadingGaps(headings, snapshotTitles)) {
    const memoCandidates = gap.memo.filter(
      (candidate) =>
        !templateByTitle.has(candidate.title) &&
        !snapshotSet.has(candidate.title),
    );
    const baseCandidates = gap.base.filter((title) => !memoSet.has(title));
    if (
      memoCandidates.length !== baseCandidates.length ||
      memoCandidates.length === 0
    ) {
      continue;
    }
    memoCandidates.forEach((candidate, position) => {
      const base = baseCandidates[position];
      if (base === undefined || !templateByTitle.has(base)) {
        return;
      }
      renameTarget.set(base, candidate.index);
    });
  }

  const result: TemplateSection[] = [];
  const consumedMemo = new Set<number>();
  for (const section of originalSections) {
    const title = section.title.trim();
    if (memoSet.has(title)) {
      result.push({
        title: section.title,
        description: section.description ?? "",
      });
      continue;
    }
    const renamedIndex = renameTarget.get(title);
    if (renamedIndex !== undefined) {
      const renamed = headings[renamedIndex] ?? title;
      result.push({ title: renamed, description: section.description ?? "" });
      consumedMemo.add(renamedIndex);
      continue;
    }
    if (snapshotSet.has(title)) {
      continue;
    }
    result.push({
      title: section.title,
      description: section.description ?? "",
    });
  }
  headings.forEach((title, index) => {
    if (templateByTitle.has(title) || consumedMemo.has(index)) {
      return;
    }
    if (snapshotSet.has(title)) {
      return;
    }
    result.push({ title, description: "" });
  });
  return result;
}

function getMemoTemplateSections(
  snapshot: SessionContentSnapshot,
  originalSections: TemplateSection[],
  options: {
    preferTemplate?: boolean;
    appliedSnapshot?: AppliedTemplateSnapshot | null;
    templateMissing?: boolean;
  } = {},
): TemplateSection[] {
  const document =
    snapshot.rawContentFormat === "markdown"
      ? md2json(snapshot.rawContent)
      : parseJsonContent(snapshot.rawContent);
  const originalByTitle = new Map(
    originalSections.map((section) => [section.title.trim(), section]),
  );
  const headings = extractSectionHeadings(document);
  if (headings.length === 0) {
    return [];
  }

  const {
    preferTemplate = false,
    appliedSnapshot = null,
    templateMissing = false,
  } = options;
  if (templateMissing) {
    // No live template (deleted or failed to load): the memo headings are the
    // only section signal. Skip the merge so snapshot-backed headings are not
    // mistaken for template removals. An existing but empty template still
    // merges, so explicitly cleared sections stay cleared.
    return headings.map((title) => ({ title, description: "" }));
  }
  if (appliedSnapshot) {
    return mergeWithAppliedSnapshot(
      headings,
      appliedSnapshot.sections,
      originalSections,
    );
  }

  // Without a snapshot, titles and positions alone cannot tell a memo
  // rename from a template edit when both sides changed the same position
  // (e.g. a memo-added heading where the template later added a section).
  // When the template was saved after the memo headings last changed, the
  // divergence most likely comes from the template edit, so the live
  // template takes authority. Otherwise a rename keeps each edited heading
  // in its template position.
  const memoTitles = new Set(headings);
  const unmatchedMemo: number[] = [];
  headings.forEach((title, index) => {
    if (!originalByTitle.has(title)) {
      unmatchedMemo.push(index);
    }
  });
  const unmatchedTemplate: number[] = [];
  originalSections.forEach((section, index) => {
    if (!memoTitles.has(section.title.trim())) {
      unmatchedTemplate.push(index);
    }
  });
  const isRename =
    !preferTemplate &&
    unmatchedMemo.length === unmatchedTemplate.length &&
    unmatchedMemo.every(
      (memoIndex, position) => memoIndex === unmatchedTemplate[position],
    );

  if (isRename) {
    const preservePositionDescriptions =
      headings.length === originalSections.length;

    return headings.map((title, index) => ({
      title,
      description:
        originalByTitle.get(title)?.description ??
        (preservePositionDescriptions
          ? originalSections[index]?.description
          : undefined) ??
        "",
    }));
  }

  // Diverged: the template gained, lost, or moved sections after the memo was
  // written, or the memo added custom headings. Use the live template as the
  // base so added sections survive regeneration, and keep memo-only headings
  // so user additions are not lost. When the template is authoritative,
  // memo-only headings that look like renames of unmatched live titles are
  // stale residue and drop instead of duplicating the renamed section.
  // Descriptions stay strings: the render validator rejects null.
  const unmatchedTemplateTitles = unmatchedTemplate.map(
    (index) => originalSections[index]?.title.trim() ?? "",
  );
  const memoOnly = headings
    .filter((title) => !originalByTitle.has(title))
    .filter(
      (title) =>
        !preferTemplate || !looksLikeRenameOf(title, unmatchedTemplateTitles),
    )
    .map((title) => ({ title, description: "" }));
  return [
    ...originalSections.map((section) => ({
      title: section.title,
      description: section.description ?? "",
    })),
    ...memoOnly,
  ];
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
