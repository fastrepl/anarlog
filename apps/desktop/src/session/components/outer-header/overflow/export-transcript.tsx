import { useMutation } from "@tanstack/react-query";
import { FileTextIcon, Loader2Icon } from "lucide-react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as listener2Commands } from "@hypr/plugin-transcription";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { useTranscriptExportSegments } from "~/session/components/note-input/transcript/export-data";

export function ExportTranscript({ sessionId }: { sessionId: string }) {
  const { data: words, isLoading } = useTranscriptExportSegments(sessionId);

  const { mutate, isPending } = useMutation({
    mutationFn: async () => {
      const result = await listener2Commands.exportToVtt(sessionId, words);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: (path) => {
      void analyticsCommands.event({
        event: "session_exported",
        format: "vtt",
        word_count: words.length,
      });
      openerCommands.openPath(path, null);
    },
  });

  return (
    <DropdownMenuItem
      onClick={(e) => {
        e.preventDefault();
        mutate();
      }}
      disabled={isPending || isLoading || words.length === 0}
      className="cursor-pointer"
    >
      {isPending || isLoading ? (
        <Loader2Icon className="animate-spin" />
      ) : (
        <FileTextIcon />
      )}
      <span>
        {isPending
          ? "Exporting..."
          : isLoading
            ? "Preparing transcript..."
            : "Export Transcript"}
      </span>
    </DropdownMenuItem>
  );
}
