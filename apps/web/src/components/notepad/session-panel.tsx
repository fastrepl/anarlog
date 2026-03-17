import { RefreshCw } from "lucide-react";
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { JSONContent, TiptapEditor } from "@hypr/tiptap/editor";
import { parseJsonContent } from "@hypr/tiptap/shared";
import "@hypr/tiptap/styles.css";
import type { Segment } from "@hypr/transcript";

import { EMPTY_MENTION_CONFIG } from "@/components/transcription/constants";
import { FloatingCTA } from "@/components/transcription/floating-cta";
import { SummaryView } from "@/components/transcription/summary-view";
import { TabButton } from "@/components/transcription/tab-button";
import { TranscriptContent } from "@/components/transcription/transcript-content";
import { useTranscriptionPipeline } from "@/hooks/use-transcription-pipeline";
import { type Store, STORE_ID, UI } from "@/store/tinybase";

const NoteEditor = lazy(() => import("@hypr/tiptap/editor"));

type ContentTab = "summary" | "memos" | "transcript";

export function SessionPanel({
  sessionId,
  readonly = false,
}: {
  sessionId: string;
  readonly?: boolean;
}) {
  const store = UI.useStore(STORE_ID) as Store | undefined;
  const editorRef = useRef<{ editor: TiptapEditor | null }>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const didAutoFocus = useRef(false);
  const [activeTab, setActiveTab] = useState<ContentTab>("summary");

  const title = UI.useCell("sessions", sessionId, "title", STORE_ID) as string;
  const fileName = UI.useCell(
    "sessions",
    sessionId,
    "file_name",
    STORE_ID,
  ) as string;
  const transcript = UI.useCell(
    "sessions",
    sessionId,
    "transcript",
    STORE_ID,
  ) as string;
  const transcriptSegmentsJson = UI.useCell(
    "sessions",
    sessionId,
    "transcript_segments",
    STORE_ID,
  ) as string;
  const summary = UI.useCell(
    "sessions",
    sessionId,
    "summary",
    STORE_ID,
  ) as string;
  const memoJson = UI.useCell(
    "sessions",
    sessionId,
    "memo",
    STORE_ID,
  ) as string;
  const pipelineStatus = UI.useCell(
    "sessions",
    sessionId,
    "pipeline_status",
    STORE_ID,
  ) as string;
  const uploadProgress = UI.useCell(
    "sessions",
    sessionId,
    "upload_progress",
    STORE_ID,
  ) as number;
  const errorMessage = UI.useCell(
    "sessions",
    sessionId,
    "error_message",
    STORE_ID,
  ) as string;
  const isSummarizing = UI.useCell(
    "sessions",
    sessionId,
    "is_summarizing",
    STORE_ID,
  ) as boolean;
  const summaryError = UI.useCell(
    "sessions",
    sessionId,
    "summary_error",
    STORE_ID,
  ) as string;

  const memoContent = useMemo<JSONContent>(
    () => parseJsonContent(memoJson),
    [memoJson],
  );

  const transcriptSegments = useMemo<Segment[] | null>(() => {
    if (!transcriptSegmentsJson) return null;
    try {
      return JSON.parse(transcriptSegmentsJson) as Segment[];
    } catch {
      return null;
    }
  }, [transcriptSegmentsJson]);

  const { handleFileSelect, handleRegenerate } = useTranscriptionPipeline(
    sessionId,
    readonly ? undefined : store,
  );

  const hasTabs =
    pipelineStatus === "done" ||
    pipelineStatus === "transcribing" ||
    pipelineStatus === "error";
  const displayTitle = fileName ? fileName.replace(/\.[^.]+$/, "") : title;

  useEffect(() => {
    if (didAutoFocus.current) return;
    if (!displayTitle) {
      titleInputRef.current?.focus();
      didAutoFocus.current = true;
    }
  }, [displayTitle]);

  const handleTitleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      store?.setCell("sessions", sessionId, "title", e.target.value);
    },
    [store, sessionId],
  );

  const handleMemoChange = useCallback(
    (content: JSONContent) => {
      store?.setCell("sessions", sessionId, "memo", JSON.stringify(content));
    },
    [store, sessionId],
  );

  const handleBodyClick = (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest("input, .tiptap-root, button")) return;
    if (activeTab === "memos" || !hasTabs) {
      editorRef.current?.editor?.commands.focus("end");
    }
  };

  return (
    <>
      <div
        className="relative cursor-text px-6 pt-6 pb-24"
        onClick={handleBodyClick}
      >
        <input
          ref={titleInputRef}
          type="text"
          placeholder="Untitled"
          value={displayTitle}
          onChange={handleTitleChange}
          className="placeholder:text-muted-foreground w-full border-none bg-transparent text-xl font-semibold focus:outline-hidden"
        />

        {errorMessage && (
          <div className="mt-3 rounded-xs border border-red-200 bg-red-50 px-4 py-2">
            <p className="text-sm text-red-600">{errorMessage}</p>
          </div>
        )}

        {hasTabs ? (
          <div className="mt-4 flex flex-col">
            <div className="flex items-center gap-1">
              <TabButton
                label="Summary"
                active={activeTab === "summary"}
                onClick={() => setActiveTab("summary")}
                trailing={
                  activeTab === "summary" && transcript && !isSummarizing ? (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRegenerate();
                      }}
                      className="ml-1 rounded p-0.5 transition-colors hover:bg-neutral-200"
                    >
                      <RefreshCw size={12} />
                    </button>
                  ) : isSummarizing ? (
                    <div className="ml-1 h-3 w-3 animate-spin rounded-full border-b border-stone-600" />
                  ) : null
                }
              />
              <TabButton
                label="Memos"
                active={activeTab === "memos"}
                onClick={() => setActiveTab("memos")}
              />
              <TabButton
                label="Transcript"
                active={activeTab === "transcript"}
                onClick={() => setActiveTab("transcript")}
              />
            </div>

            <div className="mt-2 min-h-[300px]">
              {activeTab === "summary" && (
                <SummaryView
                  summary={summary}
                  isStreaming={isSummarizing}
                  error={summaryError || null}
                  onRegenerate={handleRegenerate}
                />
              )}

              {activeTab === "memos" && (
                <Suspense fallback={null}>
                  <NoteEditor
                    ref={editorRef}
                    initialContent={memoContent}
                    handleChange={handleMemoChange}
                    mentionConfig={EMPTY_MENTION_CONFIG}
                  />
                </Suspense>
              )}

              {activeTab === "transcript" && (
                <TranscriptContent
                  transcript={transcript || null}
                  segments={transcriptSegments}
                />
              )}
            </div>
          </div>
        ) : (
          <div className="mt-4">
            <Suspense fallback={null}>
              <NoteEditor
                ref={editorRef}
                initialContent={memoContent}
                handleChange={handleMemoChange}
                mentionConfig={EMPTY_MENTION_CONFIG}
              />
            </Suspense>
          </div>
        )}
      </div>

      {(!hasTabs || pipelineStatus === "error") && !readonly && (
        <FloatingCTA
          status={pipelineStatus}
          progress={uploadProgress}
          onFileSelect={handleFileSelect}
        />
      )}
    </>
  );
}
