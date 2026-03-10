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
import { json2md } from "@hypr/tiptap/shared";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";

import { useSessionEvent } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { buildSegments, SegmentKey } from "~/stt/segment";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";
import { convertStorageHintsToRuntime } from "~/stt/speaker-hints";
import { parseTranscriptHints, parseTranscriptWords } from "~/stt/utils";

function formatDate(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleDateString("en-US", {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function formatDuration(startMs: number, endMs: number): string {
  const durationMs = endMs - startMs;
  const minutes = Math.floor(durationMs / 60000);
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;

  if (hours > 0) {
    return `${hours}h ${remainingMinutes}m`;
  }
  return `${minutes}m`;
}

export function ExportPDF({
  sessionId,
  currentView,
}: {
  sessionId: string;
  currentView: EditorView;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const queries = main.UI.useQueries(main.STORE_ID);

  const sessionTitle = main.UI.useCell(
    "sessions",
    sessionId,
    "title",
    main.STORE_ID,
  ) as string | undefined;

  const sessionCreatedAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  ) as string | undefined;

  const event = useSessionEvent(sessionId);
  const eventTitle = event?.title;

  const rawMd = main.UI.useCell(
    "sessions",
    sessionId,
    "raw_md",
    main.STORE_ID,
  ) as string | undefined;

  const enhancedNoteId = currentView.type === "enhanced" ? currentView.id : "";
  const enhancedNoteContent = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "content",
    main.STORE_ID,
  ) as string | undefined;

  const participantNames = useMemo((): string[] => {
    if (!queries) return [];

    const names: string[] = [];
    queries.forEachResultRow(
      main.QUERIES.sessionParticipantsWithDetails,
      (rowId) => {
        const participantSessionId = queries.getResultCell(
          main.QUERIES.sessionParticipantsWithDetails,
          rowId,
          "session_id",
        );
        if (participantSessionId === sessionId) {
          const name = queries.getResultCell(
            main.QUERIES.sessionParticipantsWithDetails,
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

  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );

  const transcriptItems = useMemo((): TranscriptItem[] => {
    if (!store || !transcriptIds || transcriptIds.length === 0) {
      return [];
    }

    const wordIdToIndex = new Map<string, number>();
    const collectedWords: Array<{
      id: string;
      text: string;
      start_ms: number;
      end_ms: number;
      channel: number;
    }> = [];

    const firstStartedAt = store.getCell(
      "transcripts",
      transcriptIds[0],
      "started_at",
    );

    for (const transcriptId of transcriptIds) {
      const startedAt = store.getCell(
        "transcripts",
        transcriptId,
        "started_at",
      );
      const offset =
        typeof startedAt === "number" && typeof firstStartedAt === "number"
          ? startedAt - firstStartedAt
          : 0;

      const words = parseTranscriptWords(store, transcriptId);
      for (const word of words) {
        if (word.text === undefined || word.start_ms === undefined) continue;
        collectedWords.push({
          id: word.id,
          text: word.text,
          start_ms: word.start_ms + offset,
          end_ms: (word.end_ms ?? word.start_ms) + offset,
          channel: word.channel ?? 0,
        });
      }
    }

    collectedWords.sort((a, b) => a.start_ms - b.start_ms);
    collectedWords.forEach((w, i) => wordIdToIndex.set(w.id, i));

    const storageHints = transcriptIds.flatMap((id) =>
      parseTranscriptHints(store, id),
    );
    const speakerHints = convertStorageHintsToRuntime(
      storageHints,
      wordIdToIndex,
    );

    const segments = buildSegments(collectedWords, [], speakerHints);
    const ctx = defaultRenderLabelContext(store);
    const manager = SpeakerLabelManager.fromSegments(segments, ctx);

    return segments.map((segment) => ({
      speaker: SegmentKey.renderLabel(segment.key, ctx, manager),
      text: segment.words.map((w) => w.text).join(" "),
    }));
  }, [store, transcriptIds]);

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
            enhancedMd: memoMd,
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
            transcript: null,
            metadata,
          };
        }
        case "transcript": {
          return {
            enhancedMd: "",
            transcript:
              transcriptItems.length > 0 ? { items: transcriptItems } : null,
            metadata,
          };
        }
        default:
          return {
            enhancedMd: "",
            transcript: null,
            metadata,
          };
      }
    };
  }, [
    currentView,
    rawMd,
    enhancedNoteContent,
    transcriptItems,
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
      case "transcript":
        return "Export Transcript to PDF";
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
          has_transcript:
            currentView.type === "transcript" && transcriptItems.length > 0,
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
