import { useCallback, useMemo } from "react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import type { Operations, Segment } from "../shared";
import type { Participant, SpeakerInfo } from "./utils";

export function SegmentHeader({
  segment,
  speakerInfo,
  timestamp,
  operations,
  participants,
}: {
  segment: Segment;
  speakerInfo: SpeakerInfo;
  timestamp: string;
  operations?: Operations;
  participants?: Participant[];
}) {
  const mode =
    operations && Object.keys(operations).length > 0 ? "editor" : "viewer";
  const wordIds = segment.words.filter((w) => w.id).map((w) => w.id!);

  const headerClassName = cn([
    "bg-background sticky top-0 z-20",
    "-mx-3 px-3 py-1",
    "border-b border-neutral-200",
    "text-xs font-light",
    "flex items-center justify-between",
  ]);

  const handleAssignSpeaker = useCallback(
    (humanId: string) => {
      if (wordIds.length > 0 && operations?.onAssignSpeaker) {
        operations.onAssignSpeaker(wordIds, humanId);
      }
    },
    [wordIds, operations],
  );

  const resolvedParticipants = useMemo(
    () => participants ?? [],
    [participants],
  );

  if (mode === "editor" && wordIds.length > 0) {
    return (
      <p className={headerClassName}>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <span
              style={{ color: speakerInfo.color }}
              className="cursor-pointer rounded-xs hover:bg-neutral-100"
            >
              {speakerInfo.label}
            </span>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>Assign Speaker</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                {resolvedParticipants.map((participant) => (
                  <DropdownMenuItem
                    key={participant.humanId}
                    onClick={() => handleAssignSpeaker(participant.humanId)}
                  >
                    {participant.name || participant.humanId}
                  </DropdownMenuItem>
                ))}
                {resolvedParticipants.length === 0 && (
                  <DropdownMenuItem disabled>
                    No participants available
                  </DropdownMenuItem>
                )}
              </DropdownMenuSubContent>
            </DropdownMenuSub>
          </DropdownMenuContent>
        </DropdownMenu>
        <span className="font-mono text-neutral-500">{timestamp}</span>
      </p>
    );
  }

  return (
    <p className={headerClassName}>
      <span style={{ color: speakerInfo.color }}>{speakerInfo.label}</span>
      <span className="font-mono text-neutral-500">{timestamp}</span>
    </p>
  );
}
