import {
  commands,
  type RenderTranscriptRequest,
} from "@anlg/plugin-transcription";

import type { FloatingSpeakerLabels, ListenerState } from "./route-state";

import { SegmentKeyUtils } from "~/stt/live-segment";

type LiveSegments = ListenerState["liveSegments"];

/**
 * Runs the native contextual speaker labeler for the floating panel's live
 * segments. A name depends only on the segment key, so the last answer is kept
 * per key until the labeler responds to a newer capture, exactly like the
 * transcript tab keeps previous data while a new query is in flight.
 */
export function createFloatingSpeakerLabeler(
  onLabels: (labels: FloatingSpeakerLabels) => void,
) {
  let sessionId: string | null = null;
  let labels = new Map<string, string>();
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
          const next = new Map(labels);
          for (const segment of result.data) {
            next.set(
              SegmentKeyUtils.serialize(segment.key),
              segment.speaker_label,
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
    },
  };
}
