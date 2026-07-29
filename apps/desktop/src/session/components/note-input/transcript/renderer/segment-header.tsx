import { cn } from "@anlg/utils";

import { SpeakerAssignPopover } from "./speaker-assign";
import { useSegmentColorVars } from "./utils";

import type { Segment } from "~/stt/live-segment";

export function SegmentHeader({
  segment,
  transcriptId,
  sessionId,
  label,
}: {
  segment: Segment;
  transcriptId: string;
  sessionId?: string;
  label: string;
}) {
  const colorVars = useSegmentColorVars(segment.key);
  const headerClassName = cn([
    "bg-card sticky top-0 z-20",
    "-mx-3 px-3 py-1",
    "text-xs font-light",
    "flex items-center gap-3",
    "[--segment-color:var(--segment-color-light)]",
    "dark:[--segment-color:var(--segment-color-dark)]",
  ]);

  return (
    <div className={headerClassName} style={colorVars}>
      <SpeakerAssignPopover
        segment={segment}
        transcriptId={transcriptId}
        sessionId={sessionId}
        color="var(--segment-color)"
        label={label}
      />
    </div>
  );
}
