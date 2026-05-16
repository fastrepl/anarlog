import type {
  ChannelProfile as BoundChannelProfile,
  LiveTranscriptSegment,
  RenderedTranscriptSegment,
  SegmentKey as BoundSegmentKey,
  SegmentWord as BoundSegmentWord,
} from "@hypr/plugin-transcription";

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
};

export type SegmentKey = BoundSegmentKey;
export type SegmentWord = BoundSegmentWord;
export type Segment = LiveTranscriptSegment | RenderedTranscriptSegment;
export type SegmentChannelProfile = BoundChannelProfile;

export class SpeakerLabelManager {
  private unknownSpeakerMap: Map<string, number> = new Map();
  private nextIndex = 1;

  getUnknownSpeakerNumber(key: SegmentKey): number {
    const serialized = SegmentKeyUtils.serialize(key);
    const existing = this.unknownSpeakerMap.get(serialized);
    if (existing !== undefined) {
      return existing;
    }

    const newIndex = this.nextIndex;
    this.unknownSpeakerMap.set(serialized, newIndex);
    this.nextIndex += 1;
    return newIndex;
  }

  static fromSegments(
    segments: Segment[],
    ctx?: RenderLabelContext,
  ): SpeakerLabelManager {
    const manager = new SpeakerLabelManager();
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

    if (ctx && key.channel === "DirectMic" && key.speaker_index == null) {
      return Boolean(ctx.getSelfHumanId());
    }

    return false;
  },

  renderLabel: (
    key: SegmentKey,
    ctx?: RenderLabelContext,
    manager?: SpeakerLabelManager,
  ): string => {
    if (ctx && key.speaker_human_id) {
      const human = ctx.getHumanName(key.speaker_human_id);
      if (human) {
        return human;
      }
    }

    if (ctx && key.channel === "DirectMic" && key.speaker_index == null) {
      const selfHumanId = ctx.getSelfHumanId();
      if (selfHumanId) {
        const selfHuman = ctx.getHumanName(selfHumanId);
        return selfHuman || "You";
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

export function mergeRenderedAndLiveSegments(
  renderedSegments: Segment[],
  liveSegments: Segment[],
): Segment[] {
  if (liveSegments.length === 0) {
    return renderedSegments;
  }

  if (renderedSegments.length === 0) {
    return liveSegments;
  }

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
  const renderedOnlySegments = renderedSegments.filter((segment) => {
    if (segment.id && liveSegmentIds.has(segment.id)) {
      return false;
    }

    return !segment.words.some((word) => word.id && liveWordIds.has(word.id));
  });

  return [...renderedOnlySegments, ...liveSegments];
}
