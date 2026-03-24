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

export type SegmentWord = WordLike & { isFinal: boolean; id?: string };

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

export type Segment<TWord extends SegmentWord = SegmentWord> = {
  key: SegmentKey;
  words: TWord[];
};

export type RenderLabelContext = {
  getSelfHumanId: () => string | undefined;
  getHumanName: (id: string) => string | undefined;
};

export class SpeakerLabelManager {
  private unknownSpeakerMap: Map<string, number> = new Map();
  private nextIndex = 1;

  getUnknownSpeakerNumber(key: SegmentKey): number {
    const serialized = SegmentKey.serialize(key);
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
      if (!SegmentKey.isKnownSpeaker(segment.key, ctx)) {
        manager.getUnknownSpeakerNumber(segment.key);
      }
    }
    return manager;
  }
}

export type SegmentKey = {
  readonly channel: ChannelProfile;
  readonly speaker_index?: number;
  readonly speaker_human_id?: string;
};

export const SegmentKey = {
  make: (
    params: { channel: ChannelProfile } & Partial<{
      speaker_index: number;
      speaker_human_id: string;
    }>,
  ): SegmentKey => ({ ...params }),

  hasSpeakerIdentity: (key: SegmentKey): boolean => {
    return (
      key.speaker_index !== undefined || key.speaker_human_id !== undefined
    );
  },

  equals: (a: SegmentKey, b: SegmentKey): boolean => {
    return (
      a.channel === b.channel &&
      a.speaker_index === b.speaker_index &&
      a.speaker_human_id === b.speaker_human_id
    );
  },

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

    if (ctx && key.channel === ChannelProfile.DirectMic) {
      const selfHumanId = ctx.getSelfHumanId();
      if (selfHumanId) {
        return true;
      }
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

    if (ctx && key.channel === ChannelProfile.DirectMic) {
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
      key.channel === ChannelProfile.DirectMic
        ? "A"
        : key.channel === ChannelProfile.RemoteParty
          ? "B"
          : "C";

    return key.speaker_index !== undefined
      ? `Speaker ${key.speaker_index + 1}`
      : `Speaker ${channelLabel}`;
  },
};

export type SegmentBuilderOptions = {
  maxGapMs?: number;
  numSpeakers?: number;
};

export type SpeakerIdentity = {
  speaker_index?: number;
  human_id?: string;
};

export type NormalizedWord = SegmentWord & { order: number };

export type ResolvedWordFrame = {
  word: NormalizedWord;
  identity?: SpeakerIdentity;
};

export type ProtoSegment = {
  key: SegmentKey;
  words: ResolvedWordFrame[];
};

export type SpeakerState = {
  assignmentByWordIndex: Map<number, SpeakerIdentity>;
  humanIdBySpeakerIndex: Map<number, string>;
  humanIdByChannel: Map<ChannelProfile, string>;
  lastSpeakerByChannel: Map<ChannelProfile, SpeakerIdentity>;
  completeChannels: Set<ChannelProfile>;
};

export type Operations = {
  onDeleteWord?: (wordId: string) => void;
  onAssignSpeaker?: (wordIds: string[], humanId: string) => void;
};
