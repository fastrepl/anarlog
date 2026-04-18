import { useMutation } from "@tanstack/react-query";
import { downloadDir, join } from "@tauri-apps/api/path";
import { FileTextIcon, Loader2Icon } from "lucide-react";
import { useMemo } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import {
  commands as exportCommands,
  type ExportMetadata,
  type TranscriptItem,
} from "@hypr/plugin-export";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { formatDate, formatDuration } from "./export-utils";

import { json2md } from "~/editor/markdown";
import {
  useEnhancedNoteCell,
  useSessionParticipantNames,
  useSessionCell,
  useSessionEvent,
  useTranscriptTimeRange,
} from "~/session/hooks/storage";
import type { EditorView } from "~/store/zustand/tabs/schema";

export function ExportPDF({
  sessionId,
  currentView,
}: {
  sessionId: string;
  currentView: EditorView;
}) {
  const sessionTitle = useSessionCell(sessionId, "title");
  const sessionCreatedAt = useSessionCell(sessionId, "created_at");

  const event = useSessionEvent(sessionId);
  const eventTitle = event?.title;

  const rawMd = useSessionCell(sessionId, "raw_md");

  const enhancedNoteId = currentView.type === "enhanced" ? currentView.id : "";
  const enhancedNoteContent = useEnhancedNoteCell(enhancedNoteId, "content");

  const participantNames = useSessionParticipantNames(sessionId);

  const transcriptRange = useTranscriptTimeRange(sessionId);

  const transcriptDuration = useMemo((): string | null => {
    if (
      transcriptRange.startedAt === null ||
      transcriptRange.endedAt === null
    ) {
      return null;
    }

    return formatDuration(transcriptRange.startedAt, transcriptRange.endedAt);
  }, [transcriptRange.endedAt, transcriptRange.startedAt]);

  const getExportContent = useMemo(() => {
    return (): {
      enhancedMd: string;
      memoMd: string | null;
      transcript: { items: TranscriptItem[] } | null;
      metadata: ExportMetadata | null;
    } => {
      const metadata: ExportMetadata = {
        title: sessionTitle || "Untitled",
        createdAt: sessionCreatedAt ? formatDate(sessionCreatedAt) : "",
        participants: participantNames,
        eventTitle: eventTitle || null,
        duration: transcriptDuration,
      };

      switch (currentView.type) {
        case "raw": {
          let memoMd = "";
          if (rawMd) {
            try {
              const parsed = JSON.parse(rawMd);
              memoMd = json2md(parsed);
            } catch {
              memoMd = "";
            }
          }
          return {
            enhancedMd: "",
            memoMd,
            transcript: null,
            metadata,
          };
        }
        case "enhanced": {
          let enhancedMd = "";
          if (enhancedNoteContent) {
            try {
              const parsed = JSON.parse(enhancedNoteContent);
              enhancedMd = json2md(parsed);
            } catch {
              enhancedMd = "";
            }
          }
          return {
            enhancedMd,
            memoMd: null,
            transcript: null,
            metadata,
          };
        }
        default:
          return {
            enhancedMd: "",
            memoMd: null,
            transcript: null,
            metadata,
          };
      }
    };
  }, [
    currentView,
    rawMd,
    enhancedNoteContent,
    sessionTitle,
    sessionCreatedAt,
    participantNames,
    eventTitle,
    transcriptDuration,
  ]);

  const getExportLabel = () => {
    switch (currentView.type) {
      case "raw":
        return "Export Memo to PDF";
      case "enhanced":
        return "Export Summary to PDF";
      default:
        return "Export to PDF";
    }
  };

  const { mutate, isPending } = useMutation({
    mutationFn: async () => {
      const downloadsPath = await downloadDir();
      const sanitizedTitle = (
        (sessionTitle ?? "Untitled").trim() || "Untitled"
      ).replace(/[<>:"/\\|?*]/g, "_");
      const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
      const filename = `${sanitizedTitle}_${timestamp}.pdf`;
      const path = await join(downloadsPath, filename);

      const exportContent = getExportContent();
      const result = await exportCommands.export(path, exportContent);

      if (result.status === "error") {
        throw new Error(result.error);
      }

      return path;
    },
    onSuccess: (path) => {
      if (path) {
        void analyticsCommands.event({
          event: "session_exported",
          format: "pdf",
          view_type: currentView.type,
          has_transcript: false,
          has_enhanced:
            currentView.type === "enhanced" && !!enhancedNoteContent,
          has_memo: currentView.type === "raw" && !!rawMd,
        });
        void openerCommands.revealItemInDir(path);
      }
    },
    onError: console.error,
  });

  return (
    <DropdownMenuItem
      onClick={(e) => {
        e.preventDefault();
        void mutate(null);
      }}
      disabled={isPending}
      className="cursor-pointer"
    >
      {isPending ? <Loader2Icon className="animate-spin" /> : <FileTextIcon />}
      <span>{isPending ? "Exporting..." : getExportLabel()}</span>
    </DropdownMenuItem>
  );
}
