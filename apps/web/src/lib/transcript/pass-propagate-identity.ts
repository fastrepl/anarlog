import type { ChannelProfile, ProtoSegment, SpeakerState } from "./shared";
import { SegmentKey as SegmentKeyUtils } from "./shared";

export function propagateIdentity(
  segments: ProtoSegment[],
  speakerState: SpeakerState,
): void {
  let writeIndex = 0;
  let lastKept: ProtoSegment | undefined;

  for (const segment of segments) {
    assignCompleteChannelHumanId(segment, speakerState);

    if (
      lastKept &&
      SegmentKeyUtils.equals(lastKept.key, segment.key) &&
      SegmentKeyUtils.hasSpeakerIdentity(segment.key)
    ) {
      lastKept.words.push(...segment.words);
      continue;
    }

    segments[writeIndex] = segment;
    lastKept = segment;
    writeIndex += 1;
  }

  segments.length = writeIndex;
}

function assignCompleteChannelHumanId(
  segment: ProtoSegment,
  state: SpeakerState,
): void {
  if (segment.key.speaker_human_id !== undefined) {
    return;
  }

  const channel = segment.key.channel;
  if (!state.completeChannels.has(channel)) {
    return;
  }

  const humanId = state.humanIdByChannel.get(channel);
  if (!humanId) {
    return;
  }

  const params: {
    channel: ChannelProfile;
    speaker_index?: number;
    speaker_human_id: string;
  } = {
    channel,
    speaker_human_id: humanId,
  };

  if (segment.key.speaker_index !== undefined) {
    params.speaker_index = segment.key.speaker_index;
  }

  segment.key = SegmentKeyUtils.make(params);
}
