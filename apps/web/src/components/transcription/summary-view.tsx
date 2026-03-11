import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Streamdown } from "streamdown";

import type { JSONContent } from "@hypr/tiptap/editor";
import NoteEditor from "@hypr/tiptap/editor";
import {
  EMPTY_TIPTAP_DOC,
  md2json,
  streamdownComponents,
} from "@hypr/tiptap/shared";
import "@hypr/tiptap/styles.css";
import { cn } from "@hypr/utils";

import { EMPTY_MENTION_CONFIG } from "./constants";

export function SummaryView({
  summary,
  isStreaming,
  error,
  onRegenerate,
}: {
  summary: string;
  isStreaming: boolean;
  error: string | null;
  onRegenerate: () => void;
}) {
  const [editableContent, setEditableContent] =
    useState<JSONContent>(EMPTY_TIPTAP_DOC);
  const [isEditing, setIsEditing] = useState(false);

  useEffect(() => {
    if (!isStreaming && summary) {
      try {
        const json = md2json(summary);
        setEditableContent(json);
        setIsEditing(true);
      } catch {
        setIsEditing(false);
      }
    }
  }, [isStreaming, summary]);

  if (error) {
    return (
      <div className="flex flex-col items-center gap-4 py-12 text-center">
        <p className="text-sm text-red-600">{error}</p>
        <button
          onClick={onRegenerate}
          className={cn([
            "flex items-center gap-2 px-4 py-2 text-sm",
            "rounded-lg border border-neutral-200",
            "transition-colors hover:bg-neutral-50",
          ])}
        >
          <RefreshCw size={14} />
          Retry
        </button>
      </div>
    );
  }

  if (isStreaming) {
    if (!summary) {
      return (
        <div className="py-8 text-center">
          <p className="text-sm text-neutral-500">Generating summary...</p>
        </div>
      );
    }

    return (
      <div className="pb-2">
        <Streamdown
          components={streamdownComponents}
          className={cn(["flex flex-col"])}
          caret="block"
          isAnimating={true}
        >
          {summary}
        </Streamdown>
      </div>
    );
  }

  if (isEditing && summary) {
    return (
      <NoteEditor
        key={`summary-${summary.length}`}
        initialContent={editableContent}
        handleChange={setEditableContent}
        mentionConfig={EMPTY_MENTION_CONFIG}
      />
    );
  }

  return (
    <div className="py-8 text-center">
      <p className="text-sm text-neutral-400">
        Summary will appear here after transcription
      </p>
    </div>
  );
}
