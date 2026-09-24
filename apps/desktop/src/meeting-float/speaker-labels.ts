import {
  commands,
  type RenderTranscriptRequest,
  type RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

import type { ListenerState } from "./route-state";

import { SpeakerLabelManager } from "~/stt/live-segment";
import { buildSpeakerResolutionInput } from "~/stt/useResolvedSpeakerSegments";

export type FloatingSpeakerLabel = { label: string; humanId?: string };
export type FloatingSpeakerLabels = ReadonlyMap<string, FloatingSpeakerLabel>;

// Labels the floating panel's live segments with the same native resolver the
// transcript tab uses. The latest answer wins; the previous one stays on screen
// only while a newer request is in flight, and everything resets with the session.
export function createFloatingSpeakerLabeler(onChange: () => void) {
  let sessionId: string | null = null;
  let generation = 0;
  let labels = new Map<string, FloatingSpeakerLabel>();
  // Anonymous numbers are first-seen for the whole session, like the tab, so
  // evicting old segments from the bounded live window cannot renumber a voice.
  let numbering = new SpeakerLabelManager();
  const clear = () => {
    generation += 1;
    if (labels.size > 0) {
      labels = new Map();
      onChange();
    }
  };

  return {
    get labels(): FloatingSpeakerLabels {
      return labels;
    },
    update(
      nextSessionId: string | null,
      segments: ListenerState["liveSegments"],
      request: RenderTranscriptRequest | null,
    ) {
      if (sessionId !== nextSessionId) {
        sessionId = nextSessionId;
        numbering = new SpeakerLabelManager();
      }
      const input = sessionId
        ? buildSpeakerResolutionInput(segments, request)
        : null;
      if (!input) {
        clear();
        return;
      }

      const current = ++generation;
      const ids = new Set(segments.map((segment) => segment.id));
      commands.renderTranscriptSegments(input).then(
        (result) => {
          if (current !== generation) return;
          if (result.status === "error") {
            console.error(
              "[meeting-float] speaker resolution failed",
              result.error,
            );
            return;
          }
          labels = collectLabels(result.data, ids, numbering);
          onChange();
        },
        (error) =>
          console.error("[meeting-float] speaker resolution failed", error),
      );
    },
    dispose: clear,
  };
}

function collectLabels(
  rendered: RenderedTranscriptSegment[],
  liveIds: ReadonlySet<string>,
  numbering: SpeakerLabelManager,
): Map<string, FloatingSpeakerLabel> {
  const labels = new Map<string, FloatingSpeakerLabel>();
  for (const part of rendered) {
    // A context boundary splits a segment into `<id>:<start_ms>` parts; the
    // bubble shows the latest part's speaker.
    const id = liveIds.has(part.id)
      ? part.id
      : part.id.slice(0, part.id.lastIndexOf(":"));
    if (!liveIds.has(id)) continue;
    const anonymous = !part.key.speaker_human_id && !part.provisional_speaker;
    labels.set(id, {
      label: anonymous
        ? `Speaker ${numbering.getUnknownSpeakerNumber(part.key)}`
        : part.speaker_label,
      humanId:
        part.key.speaker_human_id ??
        part.provisional_speaker?.human_id ??
        undefined,
    });
  }
  return labels;
}
