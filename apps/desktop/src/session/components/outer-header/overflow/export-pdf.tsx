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
  SESSION_PARTICIPANTS_WITH_DETAILS_QUERY,
  useEnhancedNoteCell,
  useMainQueries,
  useMainStore,
  useSessionCell,
  useSessionEvent,
  useTranscriptIdsForSession,
} from "~/session/hooks/storage";
import type { EditorView } from "~/store/zustand/tabs/schema";

export function ExportPDF({
  sessionId,
  currentView,
}: {
  sessionId: string;
  currentView: EditorView;
}) {
  const store = useMainStore();
  const queries = useMainQueries();

  const sessionTitle = useSessionCell(sessionId, "title");
  const sessionCreatedAt = useSessionCell(sessionId, "created_at");

  const event = useSessionEvent(sessionId);
  const eventTitle = event?.title;

  const rawMd = useSessionCell(sessionId, "raw_md");

  const enhancedNoteId = currentView.type === "enhanced" ? currentView.id : "";
  const enhancedNoteContent = useEnhancedNoteCell(enhancedNoteId, "content");

  const participantNames = useMemo((): string[] => {
    if (!queries) return [];

    const names: string[] = [];
    queries.forEachResultRow(
      SESSION_PARTICIPANTS_WITH_DETAILS_QUERY,
      (rowId) => {
        const participantSessionId = queries.getResultCell(
          SESSION_PARTICIPANTS_WITH_DETAILS_QUERY,
          rowId,
          "session_id",
        );
        if (participantSessionId === sessionId) {
          const name = queries.getResultCell(
            SESSION_PARTICIPANTS_WITH_DETAILS_QUERY,
            rowId,
            "human_name",
          );
          if (name && typeof name === "string") {
            names.push(name);
          }
        }
      },
    );
    return names;
  }, [queries, sessionId]);

  const transcriptIds = useTranscriptIdsForSession(sessionId);

  const transcriptDuration = useMemo((): string | null => {
    if (!store || !transcriptIds || transcriptIds.length === 0) {
      return null;
    }

    let minStartedAt: number | null = null;
    let maxEndedAt: number | null = null;

    for (const transcriptId of transcriptIds) {
      const startedAt = store.getCell(
        "transcripts",
        transcriptId,
        "started_at",
      );
      const endedAt = store.getCell("transcripts", transcriptId, "ended_at");

      if (typeof startedAt === "number") {
        if (minStartedAt === null || startedAt < minStartedAt) {
          minStartedAt = startedAt;
        }
      }
      if (typeof endedAt === "number") {
        if (maxEndedAt === null || endedAt > maxEndedAt) {
          maxEndedAt = endedAt;
        }
      }
    }

    if (minStartedAt !== null && maxEndedAt !== null) {
      return formatDuration(minStartedAt, maxEndedAt);
    }
    return null;
  }, [store, transcriptIds]);

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
