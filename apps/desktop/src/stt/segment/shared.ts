import { Schema } from "effect";

import { type Store } from "~/store/tinybase/store/main";
import { ChannelProfile, type RenderLabelContext } from "~/stt/live-segment";

export {
  ChannelProfile,
  type PartialWord,
  type RenderLabelContext,
  type RuntimeSpeakerHint,
  SpeakerLabelManager,
  type WordLike,
} from "~/stt/live-segment";

export const ChannelProfileSchema = Schema.Enums(ChannelProfile);

export const defaultRenderLabelContext = (
  store: Pick<Store, "getValue" | "getRow" | "forEachRow" | "getCell">,
  sessionId?: string | null,
): RenderLabelContext => {
  return {
    getSelfHumanId: () => {
      const selfId = store.getValue("user_id");
      return typeof selfId === "string" ? selfId : undefined;
    },
    getHumanName: (id: string) => {
      const human = store.getRow("humans", id);
      return typeof human.name === "string" ? human.name : undefined;
    },
    getParticipantHumanIds: () => {
      if (!sessionId) {
        return [];
      }

      const humanIds: string[] = [];
      store.forEachRow(
        "mapping_session_participant",
        (mappingId, _forEachCell) => {
          const mappingSessionId = store.getCell(
            "mapping_session_participant",
            mappingId,
            "session_id",
          );
          if (mappingSessionId !== sessionId) {
            return;
          }

          const humanId = store.getCell(
            "mapping_session_participant",
            mappingId,
            "human_id",
          );
          if (typeof humanId === "string" && humanId) {
            humanIds.push(humanId);
          }
        },
      );

      return [...new Set(humanIds)];
    },
  };
};
