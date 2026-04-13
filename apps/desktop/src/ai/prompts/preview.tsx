import { GripVerticalIcon } from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

type PromptTokenKind = "expression" | "statement" | "variable" | "filter";

const INLINE_TOKEN_REGEX = /({{.*?}}|{#.*?#})/g;

export function PromptTemplatePreview({
  content,
  onInsert,
}: {
  content: string;
  onInsert: (snippet: string) => void;
}) {
  const lines = content.split("\n");

  return (
    <div className="rounded-xl border border-neutral-200 bg-stone-50 p-4">
      <div className="flex flex-col gap-2">
        {lines.map((line, index) => (
          <PromptPreviewLine
            key={`${index}-${line}`}
            line={line}
            onInsert={onInsert}
          />
        ))}
      </div>
    </div>
  );
}

export function PromptInsertChip({
  label,
  snippet,
  kind,
  onInsert,
  className,
}: {
  label: string;
  snippet: string;
  kind: PromptTokenKind;
  onInsert: (snippet: string) => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      draggable
      onClick={() => onInsert(snippet)}
      onDragStart={(event) => {
        event.dataTransfer.setData("text/plain", snippet);
        event.dataTransfer.effectAllowed = "copy";
      }}
      className={cn([
        "group inline-flex items-center gap-1 rounded-full border px-2 py-1 text-xs font-medium transition-colors",
        kind === "expression" && [
          "border-sky-200 bg-sky-50 text-sky-900",
          "hover:border-sky-300 hover:bg-sky-100",
        ],
        kind === "statement" && [
          "border-amber-200 bg-amber-50 text-amber-900",
          "hover:border-amber-300 hover:bg-amber-100",
        ],
        kind === "variable" && [
          "border-neutral-200 bg-white text-neutral-700",
          "hover:border-neutral-300 hover:bg-neutral-50",
        ],
        kind === "filter" && [
          "border-emerald-200 bg-emerald-50 text-emerald-900",
          "hover:border-emerald-300 hover:bg-emerald-100",
        ],
        className,
      ])}
      title="Click to insert or drag into the editor"
    >
      <GripVerticalIcon className="h-3 w-3 opacity-40 transition-opacity group-hover:opacity-70" />
      <span className="truncate">{label}</span>
    </button>
  );
}

function PromptPreviewLine({
  line,
  onInsert,
}: {
  line: string;
  onInsert: (snippet: string) => void;
}) {
  const trimmed = line.trim();

  if (!trimmed) {
    return <div className="h-2" />;
  }

  const statement = getStatementToken(trimmed);
  if (statement) {
    return (
      <div className="flex">
        <PromptInsertChip
          label={statement.label}
          snippet={statement.snippet}
          kind="statement"
          onInsert={onInsert}
        />
      </div>
    );
  }

  const headingMatch = line.match(/^(#{1,6})\s+(.*)$/);
  if (headingMatch) {
    const level = headingMatch[1].length;
    const headingClass =
      level === 1
        ? "text-base font-semibold text-neutral-900"
        : level === 2
          ? "text-sm font-semibold text-neutral-900"
          : "text-sm font-medium text-neutral-800";

    return (
      <div className={cn(["whitespace-pre-wrap", headingClass])}>
        {renderInlineTokens(headingMatch[2], onInsert)}
      </div>
    );
  }

  return (
    <div className="text-sm whitespace-pre-wrap text-neutral-700">
      {renderInlineTokens(line, onInsert)}
    </div>
  );
}

function renderInlineTokens(
  line: string,
  onInsert: (snippet: string) => void,
): ReactNode[] {
  const tokens: ReactNode[] = [];
  let lastIndex = 0;

  for (const match of line.matchAll(INLINE_TOKEN_REGEX)) {
    if (match.index === undefined) {
      continue;
    }

    if (match.index > lastIndex) {
      tokens.push(
        <span key={`text-${match.index}`}>
          {line.slice(lastIndex, match.index)}
        </span>,
      );
    }

    const snippet = match[0];
    tokens.push(
      <PromptInsertChip
        key={`token-${match.index}`}
        label={formatExpressionLabel(snippet)}
        snippet={snippet}
        kind="expression"
        onInsert={onInsert}
        className="mx-1 align-middle"
      />,
    );

    lastIndex = match.index + snippet.length;
  }

  if (lastIndex < line.length) {
    tokens.push(<span key={`tail-${lastIndex}`}>{line.slice(lastIndex)}</span>);
  }

  return tokens.length > 0 ? tokens : [<span key="line">{line}</span>];
}

function getStatementToken(line: string) {
  if (!line.startsWith("{%") || !line.endsWith("%}")) {
    return null;
  }

  return {
    label: line.slice(2, -2).trim(),
    snippet: line,
  };
}

function formatExpressionLabel(snippet: string) {
  if (snippet.startsWith("{{")) {
    return snippet.slice(2, -2).trim();
  }

  return snippet.slice(2, -2).trim();
}
