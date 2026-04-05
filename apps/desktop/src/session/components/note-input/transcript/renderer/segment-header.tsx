import { useMemo } from "react";

import { cn } from "@hypr/utils";

import { SpeakerAssignPopover } from "./speaker-assign";
import { getTimestampRange, useSegmentColor } from "./utils";

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
  const color = useSegmentColor(segment.key);
  const label = useSpeakerLabel(segment.key, speakerLabelManager);
  const timestamp = getTimestampRange(segment);
  const headerClassName = cn([
    "bg-background sticky top-0 z-20",
    "-mx-3 px-3 py-1",
    "border-b border-neutral-200",
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
  const store = main.UI.useStore(main.STORE_ID);

  return useMemo(() => {
    if (!store) {
      return SegmentKeyUtils.renderLabel(key, undefined, manager);
    }
    const ctx = defaultRenderLabelContext(store);
    return SegmentKeyUtils.renderLabel(key, ctx, manager);
  }, [key, manager, store]);
}
