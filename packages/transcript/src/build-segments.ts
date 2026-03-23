import { collectSegments } from "./pass-build-segments";
import { normalizeWords } from "./pass-normalize-words";
import { propagateIdentity } from "./pass-propagate-identity";
import { resolveIdentities } from "./pass-resolve-speakers";
import type {
  ChannelProfile,
  ProtoSegment,
  RuntimeSpeakerHint,
  Segment,
  SegmentBuilderOptions,
  SegmentWord,
  SpeakerIdentity,
  SpeakerState,
  WordLike,
} from "./shared";

export function buildSegments<
  TFinal extends WordLike,
  TPartial extends WordLike,
>(
  finalWords: readonly TFinal[],
  partialWords: readonly TPartial[],
  speakerHints: readonly RuntimeSpeakerHint[] = [],
  options?: SegmentBuilderOptions,
): Segment[] {
  if (finalWords.length === 0 && partialWords.length === 0) {
    return [];
  }

  const resolvedOptions: SegmentBuilderOptions = options ? { ...options } : {};
  const speakerState = createSpeakerState(speakerHints, resolvedOptions);

  const words = normalizeWords(finalWords, partialWords);
  const frames = resolveIdentities(words, speakerState);
  const protoSegments = collectSegments(frames, resolvedOptions);
  propagateIdentity(protoSegments, speakerState);

  return finalizeSegments(protoSegments);
}

function createSpeakerState(
  speakerHints: readonly RuntimeSpeakerHint[],
  options?: SegmentBuilderOptions,
): SpeakerState {
  const assignmentByWordIndex = new Map<number, SpeakerIdentity>();
  const humanIdBySpeakerIndex = new Map<number, string>();
  const humanIdByChannel = new Map<ChannelProfile, string>();
  const lastSpeakerByChannel = new Map<ChannelProfile, SpeakerIdentity>();
  const completeChannels = new Set<ChannelProfile>();
  completeChannels.add(0);

  if (options?.numSpeakers === 2) {
    completeChannels.add(1);
  }

  for (const hint of speakerHints) {
    const current = assignmentByWordIndex.get(hint.wordIndex) ?? {};
    if (hint.data.type === "provider_speaker_index") {
      current.speaker_index = hint.data.speaker_index;
    } else {
      current.human_id = hint.data.human_id;
    }
    assignmentByWordIndex.set(hint.wordIndex, { ...current });

    if (current.speaker_index !== undefined && current.human_id !== undefined) {
      humanIdBySpeakerIndex.set(current.speaker_index, current.human_id);
    }
  }

  return {
    assignmentByWordIndex,
    humanIdBySpeakerIndex,
    humanIdByChannel,
    lastSpeakerByChannel,
    completeChannels,
  };
}

function finalizeSegments(segments: ProtoSegment[]): Segment[] {
  return segments.map((segment) => ({
    key: segment.key,
    words: segment.words.map(({ word }) => {
      const { order: _order, ...rest } = word;
      return rest as SegmentWord;
    }),
  }));
}
