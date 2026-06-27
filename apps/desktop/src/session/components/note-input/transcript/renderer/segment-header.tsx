import { useMemo } from "react";

import { cn } from "@hypr/utils";

import { SpeakerAssignPopover } from "./speaker-assign";
import { useSegmentColorVars } from "./utils";

import * as main from "~/store/tinybase/store/main";
import type { Segment } from "~/stt/live-segment";
import { SegmentKeyUtils, SpeakerLabelManager } from "~/stt/live-segment";
import { defaultRenderLabelContext } from "~/stt/segment/shared";

export function SegmentHeader({
  segment,
  transcriptId,
  speakerLabelManager,
}: {
  segment: Segment;
  transcriptId: string;
  speakerLabelManager?: SpeakerLabelManager;
}) {
  const colorVars = useSegmentColorVars(segment.key);
  const label = useSpeakerLabel(segment.key, speakerLabelManager);
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
        color="var(--segment-color)"
        label={label}
      />
    </div>
  );
}

function useSpeakerLabel(key: Segment["key"], manager?: SpeakerLabelManager) {
  const store = main.UI.useStore(main.STORE_ID);

  return useMemo(() => {
    if (!store) {
      return SegmentKeyUtils.renderLabel(key, undefined, manager);
    }
    const ctx = defaultRenderLabelContext(store);
    return SegmentKeyUtils.renderLabel(key, ctx, manager);
  }, [key, manager, store]);
}
