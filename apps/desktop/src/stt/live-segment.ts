import type {
  ChannelProfile as BoundChannelProfile,
  LiveTranscriptSegment,
  RenderTranscriptRequest,
  RenderedTranscriptSegment,
  SegmentKey as BoundSegmentKey,
  SegmentWord as BoundSegmentWord,
} from "@anlg/plugin-transcription";

import type { TranscriptWordMetadata } from "~/stt/timing";

export enum ChannelProfile {
  DirectMic = 0,
  RemoteParty = 1,
  MixedCapture = 2,
}

export type WordLike = {
  text: string;
  start_ms: number;
  end_ms: number;
  channel: ChannelProfile;
  metadata?: TranscriptWordMetadata | null;
};

export type PartialWord = WordLike;

type SpeakerHintData =
  | {
      type: "provider_speaker_index";
      speaker_index: number;
      provider?: string;
      channel?: number;
    }
  | { type: "user_speaker_assignment"; human_id: string };

export type RuntimeSpeakerHint = {
  wordIndex: number;
  data: SpeakerHintData;
};

export type RenderLabelContext = {
  getSelfHumanId: () => string | undefined;
  getHumanName: (id: string) => string | undefined;
  getParticipantHumanIds?: () => string[];
};

export type SegmentKey = BoundSegmentKey;
export type SegmentWord = BoundSegmentWord & {
  metadata?: TranscriptWordMetadata | null;
};
type SegmentWithWordMetadata<T extends { words: BoundSegmentWord[] }> = Omit<
  T,
  "words"
> & {
  words: SegmentWord[];
};
export type Segment =
  | SegmentWithWordMetadata<LiveTranscriptSegment>
  | SegmentWithWordMetadata<RenderedTranscriptSegment>;
export type SegmentChannelProfile = BoundChannelProfile;

const REPLACEMENT_TIME_BUCKET_MS = 1_000;

export class SpeakerLabelManager {
  private unknownSpeakerMap: Map<string, number> = new Map();
  private nextIndex = 1;

  constructor(private readonly maxUnknownSpeakerNumber?: number) {}

  getUnknownSpeakerNumber(key: SegmentKey): number {
    const serialized = SegmentKeyUtils.serialize(key);
    const existing = this.unknownSpeakerMap.get(serialized);
    if (existing !== undefined) {
      return existing;
    }

    const newIndex =
      this.maxUnknownSpeakerNumber && this.maxUnknownSpeakerNumber > 0
        ? Math.min(this.nextIndex, this.maxUnknownSpeakerNumber)
        : this.nextIndex;
    this.unknownSpeakerMap.set(serialized, newIndex);
    this.nextIndex += 1;
    return newIndex;
  }

  static fromSegments(
    segments: Segment[],
    ctx?: RenderLabelContext,
    maxUnknownSpeakerNumber?: number,
  ): SpeakerLabelManager {
    const manager = new SpeakerLabelManager(maxUnknownSpeakerNumber);
    for (const segment of segments) {
      if (!SegmentKeyUtils.isKnownSpeaker(segment.key, ctx)) {
        manager.getUnknownSpeakerNumber(segment.key);
      }
    }
    return manager;
  }
}

export const SegmentKeyUtils = {
  serialize: (key: SegmentKey): string => {
    return JSON.stringify([
      key.channel,
      key.speaker_index ?? null,
      key.speaker_human_id ?? null,
    ]);
  },

  isKnownSpeaker: (key: SegmentKey, ctx?: RenderLabelContext): boolean => {
    if (key.speaker_human_id) {
      return true;
    }

    if (ctx && key.channel === "DirectMic") {
      return Boolean(ctx.getSelfHumanId());
    }

    if (ctx && key.channel === "RemoteParty") {
      return Boolean(getUniqueRemoteParticipantHumanId(ctx));
    }

    return false;
  },

  renderLabel: (
    key: SegmentKey,
    ctx?: RenderLabelContext,
    manager?: SpeakerLabelManager,
  ): string => {
    const assignedHumanId = key.speaker_human_id;

    if (ctx && assignedHumanId != null) {
      const human = ctx.getHumanName(assignedHumanId);
      if (human) {
        return human;
      }
    }

    if (ctx && key.channel === "DirectMic" && assignedHumanId == null) {
      const selfHumanId = ctx.getSelfHumanId();
      if (selfHumanId) {
        const selfHuman = ctx.getHumanName(selfHumanId);
        return selfHuman || "You";
      }
    }

    if (ctx && key.channel === "RemoteParty" && assignedHumanId == null) {
      const remoteHumanId = getUniqueRemoteParticipantHumanId(ctx);
      if (remoteHumanId) {
        return ctx.getHumanName(remoteHumanId) || remoteHumanId;
      }
    }

    if (manager) {
      const speakerNumber = manager.getUnknownSpeakerNumber(key);
      return `Speaker ${speakerNumber}`;
    }

    const channelLabel =
      key.channel === "DirectMic"
        ? "A"
        : key.channel === "RemoteParty"
          ? "B"
          : "C";

    return key.speaker_index !== null && key.speaker_index !== undefined
      ? `Speaker ${key.speaker_index + 1}`
      : `Speaker ${channelLabel}`;
  },
};

function getUniqueRemoteParticipantHumanId(
  ctx: RenderLabelContext,
): string | undefined {
  const selfHumanId = ctx.getSelfHumanId();
  const participantHumanIds = ctx.getParticipantHumanIds?.() ?? [];
  const remoteHumanIds = [
    ...new Set(
      participantHumanIds.filter(
        (humanId) => humanId && humanId !== selfHumanId,
      ),
    ),
  ];

  return remoteHumanIds.length === 1 ? remoteHumanIds[0] : undefined;
}

export function getMaxSpeakerNumberForParticipants(
  participantHumanIds: readonly string[],
  selfHumanId?: string | null,
): number | undefined {
  const ids = new Set(participantHumanIds.filter(Boolean));
  if (selfHumanId) {
    ids.add(selfHumanId);
  }

  return ids.size > 1 ? ids.size : undefined;
}

export function mergeRenderedAndLiveSegments(
  renderedSegments: Segment[],
  liveSegments: Segment[],
  currentRequest?: RenderTranscriptRequest | null,
): Segment[] {
  const liveSegmentIds = new Set(
    liveSegments
      .map((segment) => segment.id)
      .filter((id): id is string => typeof id === "string" && id.length > 0),
  );
  const liveWordIds = new Set(
    liveSegments.flatMap((segment) =>
      segment.words
        .map((word) => word.id)
        .filter((id): id is string => typeof id === "string" && id.length > 0),
    ),
  );
  const currentWordIds = currentRequest
    ? new Set(
        currentRequest.transcripts.flatMap((transcript) =>
          transcript.words.map((word) => word.id),
        ),
      )
    : null;
  const pendingReplacementIndex = indexPendingLiveReplacements(
    liveSegments,
    currentWordIds,
  );
  const renderedOnlySegments = renderedSegments.flatMap((segment) => {
    if (segment.id && liveSegmentIds.has(segment.id)) {
      return [];
    }

    return filterSegmentWords(
      segment,
      (word) =>
        !(
          (word.id && liveWordIds.has(word.id)) ||
          (word.id && currentWordIds && !currentWordIds.has(word.id)) ||
          isPendingLiveReplacement(segment.key, word, pendingReplacementIndex)
        ),
      "persisted",
    );
  });

  return [...renderedOnlySegments, ...liveSegments].sort(
    (a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms,
  );
}

export function applyRenderRequestIdentitiesToSegments(
  segments: Segment[],
  request?: RenderTranscriptRequest | null,
): Segment[] {
  if (!request || segments.length === 0) {
    return segments;
  }

  const assignments = request.transcripts.flatMap(
    (transcript) => transcript.assignments,
  );
  if (assignments.length === 0) {
    return segments;
  }

  const completeChannels = getCompleteChannels(request);
  const assignmentsByWordId = new Map<string, string>();
  const assignmentsByChannel = new Map<SegmentChannelProfile, string>();
  const assignmentsByChannelSpeaker = new Map<string, string>();

  for (const assignment of assignments) {
    if (!assignment.human_id) {
      continue;
    }

    if (assignment.scope.kind === "words") {
      for (const wordId of assignment.scope.word_ids) {
        assignmentsByWordId.set(wordId, assignment.human_id);
      }
    } else if (assignment.scope.kind === "channel_speaker") {
      assignmentsByChannelSpeaker.set(
        channelSpeakerKey(
          assignment.scope.channel,
          assignment.scope.speaker_index,
        ),
        assignment.human_id,
      );
    } else if (completeChannels.has(assignment.scope.channel)) {
      assignmentsByChannel.set(assignment.scope.channel, assignment.human_id);
    }
  }

  return segments.flatMap((segment) => {
    const scopedHumanId =
      (typeof segment.key.speaker_index === "number"
        ? assignmentsByChannelSpeaker.get(
            channelSpeakerKey(segment.key.channel, segment.key.speaker_index),
          )
        : undefined) ??
      segment.key.speaker_human_id ??
      assignmentsByChannel.get(segment.key.channel) ??
      null;
    const runs: Array<{ humanId: string | null; words: SegmentWord[] }> = [];
    let previousHumanId = scopedHumanId;

    for (const word of segment.words) {
      let humanId =
        (word.id ? assignmentsByWordId.get(word.id) : undefined) ??
        scopedHumanId;
      if (!word.id && !word.is_final) {
        humanId = previousHumanId;
      }

      const run = runs[runs.length - 1];
      if (run?.humanId === humanId) {
        run.words.push(word);
      } else {
        runs.push({ humanId, words: [word] });
      }
      previousHumanId = humanId;
    }

    if (runs.length === 0) {
      return [segment];
    }

    if (runs.length === 1) {
      const humanId = runs[0]!.humanId;
      if ((segment.key.speaker_human_id ?? null) === humanId) {
        return [segment];
      }

      return [
        {
          ...segment,
          key: { ...segment.key, speaker_human_id: humanId },
        },
      ];
    }

    return runs.map(({ humanId, words }) => ({
      ...createSegmentFragment(
        segment,
        words,
        `identity:${humanId ?? "unassigned"}`,
      ),
      key: { ...segment.key, speaker_human_id: humanId },
    }));
  });
}

function getCompleteChannels(
  request: RenderTranscriptRequest,
): Set<SegmentChannelProfile> {
  const participantHumanIds = new Set(
    request.participant_human_ids.filter(Boolean),
  );
  if (request.self_human_id) {
    participantHumanIds.add(request.self_human_id);
  }

  return new Set([
    "DirectMic",
    ...(participantHumanIds.size === 2 ? (["RemoteParty"] as const) : []),
  ]);
}

function channelSpeakerKey(
  channel: SegmentChannelProfile,
  speakerIndex: number,
): string {
  return `${channel}:${speakerIndex}`;
}

function isPendingLiveReplacement(
  renderedKey: SegmentKey,
  renderedWord: SegmentWord,
  pendingReplacementIndex: Map<string, SegmentWord[]>,
): boolean {
  if (!renderedWord.id || renderedWord.end_ms <= renderedWord.start_ms) {
    return false;
  }

  const [startBucket, endBucket] = getReplacementBucketRange(renderedWord);
  for (let bucket = startBucket; bucket <= endBucket; bucket += 1) {
    const candidates = pendingReplacementIndex.get(
      replacementBucketKey(renderedKey, bucket),
    );
    if (
      candidates?.some((word) => {
        const overlap =
          Math.min(renderedWord.end_ms, word.end_ms) -
          Math.max(renderedWord.start_ms, word.start_ms);
        const shorterDuration = Math.min(
          renderedWord.end_ms - renderedWord.start_ms,
          word.end_ms - word.start_ms,
        );
        return overlap > 0 && overlap / shorterDuration >= 0.8;
      })
    ) {
      return true;
    }
  }
  return false;
}

function indexPendingLiveReplacements(
  liveSegments: Segment[],
  currentWordIds: Set<string> | null,
): Map<string, SegmentWord[]> {
  const index = new Map<string, SegmentWord[]>();
  if (!currentWordIds) {
    return index;
  }

  for (const segment of liveSegments) {
    for (const word of segment.words) {
      if (!word.id || currentWordIds.has(word.id)) {
        continue;
      }

      const [startBucket, endBucket] = getReplacementBucketRange(word);
      for (let bucket = startBucket; bucket <= endBucket; bucket += 1) {
        const key = replacementBucketKey(segment.key, bucket);
        const words = index.get(key);
        if (words) {
          words.push(word);
        } else {
          index.set(key, [word]);
        }
      }
    }
  }

  return index;
}

function getReplacementBucketRange(
  word: SegmentWord,
): [start: number, end: number] {
  const start = Math.floor(word.start_ms / REPLACEMENT_TIME_BUCKET_MS);
  const inclusiveEnd = Math.max(word.start_ms, word.end_ms - 1);
  return [start, Math.floor(inclusiveEnd / REPLACEMENT_TIME_BUCKET_MS)];
}

function replacementBucketKey(key: SegmentKey, bucket: number): string {
  return `${key.channel}:${key.speaker_index ?? "none"}:${bucket}`;
}

function filterSegmentWords(
  segment: Segment,
  keepWord: (word: SegmentWord) => boolean,
  fragmentKind: string,
): Segment[] {
  if (segment.words.every(keepWord)) {
    return [segment];
  }

  const fragments: Segment[] = [];
  let words: SegmentWord[] = [];
  const flushFragment = () => {
    if (words.length > 0) {
      fragments.push(createSegmentFragment(segment, words, fragmentKind));
      words = [];
    }
  };

  for (const word of segment.words) {
    if (keepWord(word)) {
      words.push(word);
    } else {
      flushFragment();
    }
  }
  flushFragment();
  return fragments;
}

function createSegmentFragment(
  segment: Segment,
  words: SegmentWord[],
  fragmentKind: string,
): Segment {
  const first = words[0]!;
  const last = words[words.length - 1]!;
  return {
    ...segment,
    id: [
      segment.id || "segment",
      fragmentKind,
      first.id ?? first.start_ms,
      last.id ?? last.end_ms,
    ].join(":"),
    start_ms: first.start_ms,
    end_ms: last.end_ms,
    text: words
      .map((word) => word.text)
      .join("")
      .trim(),
    words,
  };
}
