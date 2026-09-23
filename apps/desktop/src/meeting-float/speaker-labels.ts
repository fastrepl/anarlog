import {
  commands,
  type RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import type { FloatingSpeakerLabels, ListenerState } from "./route-state";

import {
  getMaxSpeakerNumberForParticipants,
  SegmentKeyUtils,
  SpeakerLabelManager,
} from "~/stt/live-segment";

type LiveSegments = ListenerState["liveSegments"];

const ANONYMOUS_LABEL = /^Speaker \d+$/;

/**
 * Runs the native contextual speaker labeler for the floating panel's live
 * segments. Identities come from the latest native result; anonymous numbers
 * are allocated once per key for the whole session so the bounded live window
 * cannot renumber speakers as old segments are evicted.
 */
export function createFloatingSpeakerLabeler(
  onLabels: (labels: FloatingSpeakerLabels) => void,
) {
  let sessionId: string | null = null;
  let labels = new Map<string, string>();
  let anonymous: SpeakerLabelManager | null = null;
  let generation = 0;

  return {
    get labels(): FloatingSpeakerLabels {
      return labels;
    },
    resolve(
      nextSessionId: string,
      segments: LiveSegments,
      request: RenderTranscriptRequest | null,
    ) {
      if (sessionId !== nextSessionId) {
        sessionId = nextSessionId;
        labels = new Map();
        anonymous = null;
      }
      if (!request || segments.length === 0) {
        return;
      }
      const current = ++generation;
      void commands
        .renderTranscriptSegments({
          ...request,
          preview: segments.map((segment) => ({
            ...segment,
            speaker_label: "",
            provisional_speaker: undefined,
          })),
        })
        .then((result) => {
          if (current !== generation || sessionId !== nextSessionId) return;
          if (result.status === "error") {
            console.error(
              "[meeting-float] speaker resolution failed",
              result.error,
            );
            return;
          }
          anonymous ??= new SpeakerLabelManager(
            getMaxSpeakerNumberForParticipants(
              request.participant_human_ids,
              request.self_human_id,
            ),
          );
          const next = new Map<string, string>();
          for (const segment of result.data) {
            const key = SegmentKeyUtils.serialize(segment.key);
            if (next.has(key)) continue;
            next.set(
              key,
              ANONYMOUS_LABEL.test(segment.speaker_label)
                ? `Speaker ${anonymous.getUnknownSpeakerNumber(segment.key)}`
                : segment.speaker_label,
            );
          }
          labels = next;
          onLabels(labels);
        })
        .catch((error) => {
          console.error("[meeting-float] speaker resolution failed", error);
        });
    },
    reset() {
      generation++;
      sessionId = null;
      labels = new Map();
      anonymous = null;
    },
  };
}
