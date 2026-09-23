import {
  commands,
  type RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import type { FloatingSpeakerLabels, ListenerState } from "./route-state";

import {
  getMaxSpeakerNumberForParticipants,
  SegmentKeyUtils,
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
  let anonymous = new Map<string, number>();
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
        anonymous = new Map();
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
          const max = getMaxSpeakerNumberForParticipants(
            request.participant_human_ids,
            request.self_human_id,
          );
          const next = new Map<string, string>();
          for (const segment of result.data) {
            if (!ANONYMOUS_LABEL.test(segment.speaker_label)) {
              next.set(
                SegmentKeyUtils.serialize(segment.key),
                segment.speaker_label,
              );
            }
          }
          for (const segment of result.data) {
            const key = SegmentKeyUtils.serialize(segment.key);
            if (next.has(key)) continue;
            let number = anonymous.get(key);
            if (number === undefined) {
              number = anonymous.size + 1;
              anonymous.set(key, number);
            }
            next.set(key, `Speaker ${max ? Math.min(number, max) : number}`);
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
      anonymous = new Map();
    },
  };
}
