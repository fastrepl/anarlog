import { useMutation } from "@tanstack/react-query";
import { FileTextIcon, Loader2Icon } from "lucide-react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as listener2Commands } from "@hypr/plugin-listener2";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { getTranscriptExportData } from "./export-transcript-data";

import * as main from "~/store/tinybase/store/main";

export function ExportTranscript({ sessionId }: { sessionId: string }) {
  const store = main.UI.useStore(main.STORE_ID);

  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );

  const { mutate, isPending } = useMutation({
    mutationFn: async () => {
      if (!store || !transcriptIds || transcriptIds.length === 0) {
        throw new Error("No transcript available");
      }

      const { vttWords } = getTranscriptExportData(store, transcriptIds);
      const result = await listener2Commands.exportToVtt(sessionId, vttWords);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return {
        path: result.data,
        wordCount: vttWords.length,
      };
    },
    onSuccess: ({ path, wordCount }) => {
      void analyticsCommands.event({
        event: "session_exported",
        format: "vtt",
        word_count: wordCount,
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
      disabled={
        isPending || !store || !transcriptIds || transcriptIds.length === 0
      }
      className="cursor-pointer"
    >
      {isPending ? <Loader2Icon className="animate-spin" /> : <FileTextIcon />}
      <span>{isPending ? "Exporting..." : "Export Transcript"}</span>
    </DropdownMenuItem>
  );
}
