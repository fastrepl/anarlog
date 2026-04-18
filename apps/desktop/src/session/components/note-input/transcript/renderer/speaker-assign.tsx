import { useCallback, useMemo, useState } from "react";

import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import {
  useAssignTranscriptSpeaker,
  useSessionParticipantPeople,
  useTranscriptSessionId,
} from "~/session/hooks/storage";
import type { Segment } from "~/stt/live-segment";

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
  const assignSpeaker = useAssignTranscriptSpeaker();
  const isSelf = segment.key.channel === "DirectMic";

  const sessionId = useTranscriptSessionId(transcriptId);

  const handleAssign = useCallback(
    (humanId: string) => {
      if (segment.words.length === 0) return;
      const anchorWordId = segment.words[0].id;
      if (!anchorWordId) return;
      assignSpeaker({
        transcriptId,
        segmentKey: segment.key,
        humanId,
        anchorWordId,
      });
      setOpen(false);
    },
    [assignSpeaker, transcriptId, segment.key, segment.words],
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
  const participantsBySession = useSessionParticipantPeople(sessionId ?? "");

  const participants = useMemo(() => {
    return participantsBySession;
  }, [participantsBySession]);

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
