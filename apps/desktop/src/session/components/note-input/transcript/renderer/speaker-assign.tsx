import { useCallback, useMemo, useState } from "react";

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";
import type { Segment } from "~/stt/live-segment";
import { upsertSpeakerAssignment } from "~/stt/utils";

export function SpeakerAssignPopover({
  segment,
  transcriptId,
  color,
  label,
}: {
  segment: Segment;
  transcriptId: string;
  color: string;
  label: string;
}) {
  const [open, setOpen] = useState(false);
  const store = main.UI.useStore(main.STORE_ID);
  const isSelf = segment.key.channel === "DirectMic";

  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  ) as string | undefined;

  const handleAssign = useCallback(
    (humanId: string) => {
      if (!store || segment.words.length === 0) return;
      const anchorWordId = segment.words[0].id;
      if (!anchorWordId) return;
      upsertSpeakerAssignment(
        store,
        transcriptId,
        segment.key,
        humanId,
        anchorWordId,
      );
      setOpen(false);
    },
    [store, transcriptId, segment.key, segment.words],
  );

  if (isSelf) {
    return <span style={{ color }}>{label}</span>;
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn([
            "-ml-1 cursor-pointer rounded-xs px-1",
            "transition-colors hover:bg-neutral-100",
          ])}
          style={{ color }}
        >
          {label}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-56 p-0"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <ParticipantList sessionId={sessionId} onSelect={handleAssign} />
      </PopoverContent>
    </Popover>
  );
}

function ParticipantList({
  sessionId,
  onSelect,
}: {
  sessionId: string | undefined;
  onSelect: (humanId: string) => void;
}) {
  const queries = main.UI.useQueries(main.STORE_ID);

  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId ?? "",
    main.STORE_ID,
  ) as string[];

  const participants = useMemo(() => {
    if (!queries) return [];
    return mappingIds
      .map((mappingId) => {
        const result = queries.getResultRow(
          main.QUERIES.sessionParticipantsWithDetails,
          mappingId,
        );
        if (!result?.human_id) return null;
        const name = (result.human_name as string) || "";
        return { id: result.human_id as string, name };
      })
      .filter((p): p is { id: string; name: string } => p !== null);
  }, [mappingIds, queries]);

  if (participants.length === 0) {
    return (
      <p className="px-3 py-2 text-xs text-neutral-400">No participants</p>
    );
  }

  return (
    <div className="max-h-48 overflow-auto py-1">
      {participants.map((p) => (
        <button
          key={p.id}
          type="button"
          className={cn([
            "w-full px-3 py-1.5 text-left text-sm",
            "hover:bg-neutral-100",
          ])}
          onClick={() => onSelect(p.id)}
        >
          {p.name}
        </button>
      ))}
    </div>
  );
}
