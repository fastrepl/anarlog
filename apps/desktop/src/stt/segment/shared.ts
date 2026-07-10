import { Schema } from "effect";

import { ChannelProfile } from "~/stt/live-segment";

export {
  ChannelProfile,
  type PartialWord,
  type RenderLabelContext,
  type RuntimeSpeakerHint,
  SpeakerLabelManager,
  type WordLike,
} from "~/stt/live-segment";

export const ChannelProfileSchema = Schema.Enums(ChannelProfile);
