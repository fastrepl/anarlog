import { useCallback, useMemo, useState } from "react";

import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import {
  SESSION_PARTICIPANTS_WITH_DETAILS_QUERY,
  useMainQueries,
  useMainStore,
  useSessionParticipantMappingIds,
  useTranscriptSessionId,
} from "~/session/hooks/storage";
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
  const store = useMainStore();
  const isSelf = segment.key.channel === "DirectMic";

  const sessionId = useTranscriptSessionId(transcriptId);

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
        variant="app"
        align="start"
        className="w-56"
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
  const queries = useMainQueries();
  const mappingIds = useSessionParticipantMappingIds(sessionId ?? "");

  const participants = useMemo(() => {
    if (!queries) return [];
    return mappingIds
      .map((mappingId) => {
        const result = queries.getResultRow(
          SESSION_PARTICIPANTS_WITH_DETAILS_QUERY,
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
      <AppFloatingPanel>
        <p className="px-3 py-2 text-xs text-neutral-400">No participants</p>
      </AppFloatingPanel>
    );
  }

  return (
    <AppFloatingPanel className="max-h-48 overflow-auto py-1">
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
    </AppFloatingPanel>
  );
}
