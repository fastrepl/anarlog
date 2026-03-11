import { useMutation } from "@tanstack/react-query";
import { downloadDir, join } from "@tauri-apps/api/path";
import { useMemo, useState } from "react";
import { createPortal } from "react-dom";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import {
  commands as exportCommands,
  type ExportMetadata,
  type TranscriptItem,
} from "@hypr/plugin-export";
import { commands as fs2Commands } from "@hypr/plugin-fs2";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { json2md } from "@hypr/tiptap/shared";
import { cn } from "@hypr/utils";

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

type FileFormat = "pdf" | "txt" | "md" | "org";

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

function markdownToText(content: string): string {
  return content
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1 ($2)")
    .replace(/^\s*[-*+]\s+/gm, "• ")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/\*\*(.*?)\*\*/g, "$1")
    .replace(/\*(.*?)\*/g, "$1")
    .replace(/__(.*?)__/g, "$1")
    .replace(/_(.*?)_/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function markdownToOrg(content: string): string {
  return content
    .replace(/^(#{1,6})\s+/gm, (_match, hashes: string) => {
      return `${"*".repeat(hashes.length)} `;
    })
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, "[[$2][$1]]")
    .replace(/\*\*(.*?)\*\*/g, "*$1*")
    .replace(/__(.*?)__/g, "*$1*")
    .replace(/`([^`]+)`/g, "~$1~")
    .trim();
}

export function ExportModal({
  sessionId,
  currentView,
  open,
  onOpenChange,
}: {
  sessionId: string;
  currentView: EditorView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [format, setFormat] = useState<FileFormat>("pdf");
  const [includeSummary, setIncludeSummary] = useState(true);
  const [includeTranscript, setIncludeTranscript] = useState(false);

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

  const getSummaryMd = (): string => {
    if (!enhancedNoteContent) return "";
    try {
      const parsed = JSON.parse(enhancedNoteContent);
      return json2md(parsed);
    } catch {
      return "";
    }
  };

  const getTranscriptText = (): string => {
    if (transcriptItems.length === 0) return "";
    return transcriptItems
      .map((item) => {
        const speaker = item.speaker ? `${item.speaker}: ` : "";
        return `${speaker}${item.text}`;
      })
      .join("\n\n");
  };

  const buildMdContent = (): string => {
    const sections: string[] = [];
    const title = sessionTitle || "Untitled";
    sections.push(`# ${title}`);

    if (sessionCreatedAt) {
      sections.push(`- Created: ${formatDate(sessionCreatedAt)}`);
    }

    if (participantNames.length > 0) {
      sections.push(`- Participants: ${participantNames.join(", ")}`);
    }

    if (transcriptDuration) {
      sections.push(`- Duration: ${transcriptDuration}`);
    }

    if (includeSummary) {
      const summary = getSummaryMd();
      if (summary) {
        sections.push("");
        sections.push("## Summary");
        sections.push(summary);
      }
    }

    if (includeTranscript) {
      const transcript = getTranscriptText();
      if (transcript) {
        sections.push("");
        sections.push("## Transcript");
        sections.push(transcript);
      }
    }

    return sections.join("\n");
  };

  const buildTxtContent = (): string => {
    const sections: string[] = [];
    const title = sessionTitle || "Untitled";
    sections.push(title);
    sections.push("=".repeat(title.length));

    if (sessionCreatedAt) {
      sections.push(formatDate(sessionCreatedAt));
    }

    if (participantNames.length > 0) {
      sections.push(`Participants: ${participantNames.join(", ")}`);
    }

    if (transcriptDuration) {
      sections.push(`Duration: ${transcriptDuration}`);
    }

    if (includeSummary) {
      const summary = getSummaryMd();
      if (summary) {
        sections.push("");
        sections.push("Summary");
        sections.push("-".repeat(7));
        sections.push(markdownToText(summary));
      }
    }

    if (includeTranscript) {
      const transcript = getTranscriptText();
      if (transcript) {
        sections.push("");
        sections.push("Transcript");
        sections.push("-".repeat(10));
        sections.push(transcript);
      }
    }

    return sections.join("\n");
  };

  const buildOrgContent = (): string => {
    const sections: string[] = [];
    const title = sessionTitle || "Untitled";
    sections.push(`#+TITLE: ${title}`);

    if (sessionCreatedAt) {
      sections.push(`#+DATE: ${formatDate(sessionCreatedAt)}`);
    }

    sections.push("");
    sections.push("* Metadata");

    if (sessionCreatedAt) {
      sections.push(`- Created :: ${formatDate(sessionCreatedAt)}`);
    }

    if (participantNames.length > 0) {
      sections.push(`- Participants :: ${participantNames.join(", ")}`);
    }

    if (transcriptDuration) {
      sections.push(`- Duration :: ${transcriptDuration}`);
    }

    if (includeSummary) {
      const summary = getSummaryMd();
      if (summary) {
        sections.push("");
        sections.push("* Summary");
        sections.push(markdownToOrg(summary));
      }
    }

    if (includeTranscript) {
      const transcript = getTranscriptText();
      if (transcript) {
        sections.push("");
        sections.push("* Transcript");
        sections.push(transcript);
      }
    }

    return sections.join("\n");
  };

  const buildPdfContent = (): {
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

    const parts: string[] = [];

    if (includeSummary) {
      const summary = getSummaryMd();
      if (summary) parts.push(summary);
    }

    return {
      enhancedMd: parts.join("\n\n"),
      transcript:
        includeTranscript && transcriptItems.length > 0
          ? { items: transcriptItems }
          : null,
      metadata,
    };
  };

  const { mutate, isPending } = useMutation({
    mutationFn: async () => {
      const downloadsPath = await downloadDir();
      const sanitizedTitle = (
        (sessionTitle ?? "Untitled").trim() || "Untitled"
      ).replace(/[<>:"/\\|?*]/g, "_");
      const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
      const filename = `${sanitizedTitle}_${timestamp}.${format}`;
      const path = await join(downloadsPath, filename);

      if (format === "pdf") {
        const exportContent = buildPdfContent();
        const result = await exportCommands.export(path, exportContent);
        if (result.status === "error") {
          throw new Error(result.error);
        }
      } else {
        const textContent =
          format === "md"
            ? buildMdContent()
            : format === "org"
              ? buildOrgContent()
              : buildTxtContent();
        const result = await fs2Commands.writeTextFile(path, textContent);
        if (result.status === "error") {
          throw new Error(result.error);
        }
      }

      return path;
    },
    onSuccess: (path) => {
      if (path) {
        void analyticsCommands.event({
          event: "session_exported",
          format,
          include_summary: includeSummary,
          include_transcript: includeTranscript,
        });
        void openerCommands.revealItemInDir(path);
      }
      onOpenChange(false);
    },
    onError: console.error,
  });

  const hasAnyContentSelected = includeSummary || includeTranscript;
  if (!open) {
    return null;
  }

  return createPortal(
    <div
      className="fixed inset-0 z-50 bg-black/20 backdrop-blur-xs"
      onClick={() => onOpenChange(false)}
    >
      <div
        className="absolute top-1/2 left-1/2 w-full max-w-xs -translate-x-1/2 -translate-y-1/2 px-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div
          className={cn([
            "rounded-xl border border-neutral-200/80 bg-[#faf8f5]",
            "shadow-[0_25px_50px_-12px_rgba(0,0,0,0.25)]",
            "flex flex-col gap-4 p-5 text-center",
          ])}
        >
          <div className="flex flex-col gap-1">
            <h2 className="text-base font-semibold">Export</h2>
            <p className="text-sm text-neutral-500">
              Choose a file format and what to include.
            </p>
          </div>

          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <span className="text-sm font-medium">File format</span>
              <div className="flex justify-center gap-4">
                {(["pdf", "txt", "md", "org"] as const).map((f) => (
                  <label
                    key={f}
                    className="flex cursor-pointer items-center gap-1.5 text-sm"
                  >
                    <input
                      type="radio"
                      name="export-format"
                      checked={format === f}
                      onChange={() => setFormat(f)}
                      className="accent-stone-800"
                    />
                    {f === "md"
                      ? "Markdown"
                      : f === "org"
                        ? "Org"
                        : f.toUpperCase()}
                  </label>
                ))}
              </div>
            </div>

            <div className="flex flex-col gap-2">
              <span className="text-sm font-medium">Include</span>
              <div className="flex justify-center gap-4">
                {(
                  [
                    ["Summary", includeSummary, setIncludeSummary],
                    ["Transcript", includeTranscript, setIncludeTranscript],
                  ] as const
                ).map(([label, checked, setter]) => (
                  <label
                    key={label}
                    className="flex cursor-pointer items-center gap-1.5 text-sm"
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={(e) => setter(e.target.checked)}
                      className="accent-stone-800"
                    />
                    {label}
                  </label>
                ))}
              </div>
            </div>
          </div>

          <button
            onClick={() => mutate(null)}
            disabled={isPending || !hasAnyContentSelected}
            className="h-10 w-full rounded-full border-2 border-stone-600 bg-stone-800 text-sm font-medium text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200 hover:bg-stone-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isPending ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
