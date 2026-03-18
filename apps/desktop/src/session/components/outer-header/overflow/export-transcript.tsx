import { useMutation } from "@tanstack/react-query";
import { FileTextIcon, Loader2Icon } from "lucide-react";
import { useMemo } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as listener2Commands } from "@hypr/plugin-listener2";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { buildTranscriptExportSegments } from "~/session/components/note-input/transcript/export-data";
import * as main from "~/store/tinybase/store/main";

export function ExportTranscript({ sessionId }: { sessionId: string }) {
  const store = main.UI.useStore(main.STORE_ID);

  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );

  const words = useMemo(() => {
    if (!store || !transcriptIds || transcriptIds.length === 0) {
      return [];
    }

    return buildTranscriptExportSegments(store, transcriptIds);
  }, [store, transcriptIds]);

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
      disabled={isPending || words.length === 0}
      className="cursor-pointer"
    >
      {isPending ? <Loader2Icon className="animate-spin" /> : <FileTextIcon />}
      <span>{isPending ? "Exporting..." : "Export Transcript"}</span>
    </DropdownMenuItem>
  );
}
