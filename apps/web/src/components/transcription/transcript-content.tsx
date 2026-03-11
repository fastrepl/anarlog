import { useMemo } from "react";

import {
  type Segment,
  type SegmentKey,
  SegmentKey as SegmentKeyUtils,
  SpeakerLabelManager,
} from "@hypr/transcript";
import { getSegmentColor, SegmentsList } from "@hypr/transcript/ui";

export function TranscriptContent({
  transcript,
  segments,
}: {
  transcript?: string | null;
  segments?: Segment[] | null;
}) {
  if (segments && segments.length > 0) {
    return <StructuredTranscript segments={segments} />;
  }

  if (!transcript) {
    return (
      <div className="py-8 text-center">
        <div className="flex flex-col items-center gap-3">
          <div className="h-6 w-6 animate-spin rounded-full border-b-2 border-stone-600" />
          <p className="text-sm text-neutral-500">Transcribing...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="prose prose-sm max-w-none">
      <p className="leading-relaxed whitespace-pre-wrap text-neutral-700">
        {transcript}
      </p>
    </div>
  );
}

function StructuredTranscript({ segments }: { segments: Segment[] }) {
  const speakerLabelManager = useMemo(
    () => SpeakerLabelManager.fromSegments(segments),
    [segments],
  );

  const speakerLabelResolver = useMemo(() => {
    return (key: SegmentKey) => {
      const label = SegmentKeyUtils.renderLabel(
        key,
        undefined,
        speakerLabelManager,
      );
      const color = getSegmentColor(key);
      return { label, color };
    };
  }, [speakerLabelManager]);

  return (
    <SegmentsList
      segments={segments}
      speakerLabelResolver={speakerLabelResolver}
    />
  );
}
