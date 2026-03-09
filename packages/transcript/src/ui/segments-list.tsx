import { memo } from "react";

import { cn } from "@hypr/utils";

import type { Operations, Segment, SegmentKey, SegmentWord } from "../shared";
import { SegmentRenderer } from "./segment-renderer";
import type { AudioState, HighlightSegment, Participant } from "./utils";
import { getTimestampRange } from "./utils";

export type SegmentsListProps = {
  segments: Segment[];
  editable?: boolean;
  offsetMs?: number;
  audioState?: AudioState;
  speakerLabelResolver: (key: SegmentKey) => { label: string; color: string };
  operations?: Operations;
  participants?: Participant[];
  getSearchHighlights?: (
    word: SegmentWord,
  ) => { segments: HighlightSegment[]; isActive: boolean } | undefined;
  onWordContextMenu?: (word: SegmentWord, event: React.MouseEvent) => void;
  createSegmentKey?: (segment: Segment, index: number) => string;
};

export const SegmentsList = memo(
  ({
    segments,
    editable = false,
    offsetMs = 0,
    audioState,
    speakerLabelResolver,
    operations,
    participants,
    getSearchHighlights,
    onWordContextMenu,
    createSegmentKey: createKey,
  }: SegmentsListProps) => {
    return (
      <div>
        {segments.map((segment, index) => {
          const key = createKey
            ? createKey(segment, index)
            : defaultSegmentKey(segment, index);
          const speakerInfo = speakerLabelResolver(segment.key);
          const timestamp = getTimestampRange(segment);

          return (
            <div key={key} className={cn([index > 0 && "pt-8"])}>
              <SegmentRenderer
                editable={editable}
                segment={segment}
                offsetMs={offsetMs}
                operations={operations}
                speakerInfo={speakerInfo}
                timestamp={timestamp}
                participants={participants}
                audioState={audioState}
                getSearchHighlights={getSearchHighlights}
                onWordContextMenu={onWordContextMenu}
              />
            </div>
          );
        })}
      </div>
    );
  },
);

function defaultSegmentKey(segment: Segment, index: number): string {
  const firstId = segment.words[0]?.id;
  if (firstId) return firstId;
  return `segment-${index}-${segment.key.channel}-${segment.key.speaker_index ?? "x"}`;
}
