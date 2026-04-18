import { useMemo } from "react";

import { cn } from "@hypr/utils";

import { SpeakerAssignPopover } from "./speaker-assign";
import { getTimestampRange, useSegmentColor } from "./utils";

import { useTranscriptLabelContext } from "~/session/hooks/storage";
import type { Segment } from "~/stt/live-segment";
import { SegmentKeyUtils, SpeakerLabelManager } from "~/stt/live-segment";

export function SegmentHeader({
  segment,
  transcriptId,
  speakerLabelManager,
}: {
  segment: Segment;
  transcriptId: string;
  speakerLabelManager?: SpeakerLabelManager;
}) {
  const color = useSegmentColor(segment.key);
  const label = useSpeakerLabel(segment.key, speakerLabelManager);
  const timestamp = getTimestampRange(segment);
  const headerClassName = cn([
    "sticky top-0 z-20 bg-neutral-50",
    "-mx-3 px-3 py-1",
    "text-xs font-light",
    "flex items-center justify-between",
  ]);

  return (
    <p className={headerClassName}>
      <SpeakerAssignPopover
        segment={segment}
        transcriptId={transcriptId}
        color={color}
        label={label}
      />
      <span className="font-mono text-neutral-500">{timestamp}</span>
    </p>
  );
}

function useSpeakerLabel(key: Segment["key"], manager?: SpeakerLabelManager) {
  const labelContext = useTranscriptLabelContext();

  return useMemo(() => {
    if (!labelContext) {
      return SegmentKeyUtils.renderLabel(key, undefined, manager);
    }
    return SegmentKeyUtils.renderLabel(key, labelContext, manager);
  }, [key, labelContext, manager]);
}
