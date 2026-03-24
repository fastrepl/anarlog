import type {
  ChannelProfile,
  ProtoSegment,
  ResolvedWordFrame,
  SegmentBuilderOptions,
  SegmentKey,
  SpeakerIdentity,
} from "./shared";
import { SegmentKey as SegmentKeyUtils } from "./shared";

function createSegmentKeyFromIdentity(
  channel: ChannelProfile,
  identity?: SpeakerIdentity,
): SegmentKey {
  const params: {
    channel: ChannelProfile;
    speaker_index?: number;
    speaker_human_id?: string;
  } = { channel };

  if (identity?.speaker_index !== undefined) {
    params.speaker_index = identity.speaker_index;
  }

  if (identity?.human_id !== undefined) {
    params.speaker_human_id = identity.human_id;
  }

  return SegmentKeyUtils.make(params);
}

export function collectSegments(
  frames: ResolvedWordFrame[],
  options?: SegmentBuilderOptions,
): ProtoSegment[] {
  const segments: ProtoSegment[] = [];
  const lastSegmentByChannel = new Map<ChannelProfile, ProtoSegment>();
  const maxGapMs = options?.maxGapMs ?? 2000;

  for (const frame of frames) {
    const key = determineKey(frame, lastSegmentByChannel);
    const last = segments[segments.length - 1];

    if (
      last &&
      SegmentKeyUtils.equals(last.key, key) &&
      frame.word.start_ms - last.words[last.words.length - 1].word.end_ms <=
        maxGapMs
    ) {
      last.words.push(frame);
    } else {
      const segment: ProtoSegment = { key, words: [frame] };
      segments.push(segment);
      lastSegmentByChannel.set(key.channel, segment);
    }
  }

  return segments;
}

function determineKey(
  frame: ResolvedWordFrame,
  lastSegmentByChannel: Map<ChannelProfile, ProtoSegment>,
): SegmentKey {
  if (!frame.word.isFinal) {
    const prev = lastSegmentByChannel.get(frame.word.channel);
    if (prev) {
      return prev.key;
    }
  }

  return createSegmentKeyFromIdentity(frame.word.channel, frame.identity);
}
