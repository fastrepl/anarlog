import { useMemo } from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { cn } from "@hypr/utils";

export interface ContentAuditPreview {
  model: string;
  originalContent: string;
  revisedContent: string;
  summary: string[];
}

type DiffLine =
  | {
      type: "same" | "added" | "removed";
      text: string;
      leftLine: number | null;
      rightLine: number | null;
    }
  | {
      type: "context";
      text: string;
      leftLine: null;
      rightLine: null;
    };

function buildLineDiff(originalContent: string, revisedContent: string) {
  const originalLines = originalContent.split("\n");
  const revisedLines = revisedContent.split("\n");
  const matrix = Array.from({ length: originalLines.length + 1 }, () =>
    Array.from<number>({ length: revisedLines.length + 1 }).fill(0),
  );

  for (let leftIndex = originalLines.length - 1; leftIndex >= 0; leftIndex--) {
    for (
      let rightIndex = revisedLines.length - 1;
      rightIndex >= 0;
      rightIndex--
    ) {
      matrix[leftIndex][rightIndex] =
        originalLines[leftIndex] === revisedLines[rightIndex]
          ? matrix[leftIndex + 1][rightIndex + 1] + 1
          : Math.max(
              matrix[leftIndex + 1][rightIndex],
              matrix[leftIndex][rightIndex + 1],
            );
    }
  }

  const lines: DiffLine[] = [];
  let leftIndex = 0;
  let rightIndex = 0;
  let leftLine = 1;
  let rightLine = 1;

  while (leftIndex < originalLines.length && rightIndex < revisedLines.length) {
    if (originalLines[leftIndex] === revisedLines[rightIndex]) {
      lines.push({
        type: "same",
        text: originalLines[leftIndex],
        leftLine,
        rightLine,
      });
      leftIndex++;
      rightIndex++;
      leftLine++;
      rightLine++;
      continue;
    }

    if (
      matrix[leftIndex + 1][rightIndex] >= matrix[leftIndex][rightIndex + 1]
    ) {
      lines.push({
        type: "removed",
        text: originalLines[leftIndex],
        leftLine,
        rightLine: null,
      });
      leftIndex++;
      leftLine++;
      continue;
    }

    lines.push({
      type: "added",
      text: revisedLines[rightIndex],
      leftLine: null,
      rightLine,
    });
    rightIndex++;
    rightLine++;
  }

  while (leftIndex < originalLines.length) {
    lines.push({
      type: "removed",
      text: originalLines[leftIndex],
      leftLine,
      rightLine: null,
    });
    leftIndex++;
    leftLine++;
  }

  while (rightIndex < revisedLines.length) {
    lines.push({
      type: "added",
      text: revisedLines[rightIndex],
      leftLine: null,
      rightLine,
    });
    rightIndex++;
    rightLine++;
  }

  return lines;
}

function collapseDiff(lines: DiffLine[], contextSize = 2) {
  const collapsed: DiffLine[] = [];

  for (let index = 0; index < lines.length; ) {
    if (lines[index].type !== "same") {
      collapsed.push(lines[index]);
      index++;
      continue;
    }

    let endIndex = index;
    while (endIndex < lines.length && lines[endIndex].type === "same") {
      endIndex++;
    }

    const runLength = endIndex - index;
    if (runLength <= contextSize * 2 + 1) {
      collapsed.push(...lines.slice(index, endIndex));
      index = endIndex;
      continue;
    }

    collapsed.push(...lines.slice(index, index + contextSize));
    collapsed.push({
      type: "context",
      text: `${runLength - contextSize * 2} unchanged lines`,
      leftLine: null,
      rightLine: null,
    });
    collapsed.push(...lines.slice(endIndex - contextSize, endIndex));
    index = endIndex;
  }

  return collapsed;
}

function formatLineNumber(value: number | null) {
  return value === null ? "" : String(value);
}

export function ContentAuditReviewDialog({
  open,
  preview,
  onOpenChange,
  onApply,
}: {
  open: boolean;
  preview: ContentAuditPreview | null;
  onOpenChange: (open: boolean) => void;
  onApply: () => void;
}) {
  const diffLines = useMemo(() => {
    if (!preview) {
      return [];
    }

    return collapseDiff(
      buildLineDiff(preview.originalContent, preview.revisedContent),
    );
  }, [preview]);

  const stats = useMemo(() => {
    return diffLines.reduce(
      (acc, line) => {
        if (line.type === "added") {
          acc.added += 1;
        }
        if (line.type === "removed") {
          acc.removed += 1;
        }
        return acc;
      },
      { added: 0, removed: 0 },
    );
  }, [diffLines]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="content-admin-chrome h-[85vh] max-w-6xl overflow-hidden p-0">
        <DialogHeader className="border-b border-neutral-200 px-6 py-5 pr-12">
          <DialogTitle>Review Audit Changes</DialogTitle>
          <DialogDescription className="text-neutral-500">
            {preview
              ? `The audit ran on ${preview.model}. Review the diff before applying the revised draft to the editor.`
              : "Review the revised draft before applying it to the editor."}
          </DialogDescription>
        </DialogHeader>

        {preview && (
          <div className="flex min-h-0 flex-1 flex-col">
            <div className="border-b border-neutral-200 px-6 py-4">
              <div className="flex flex-wrap items-center gap-2 font-mono text-xs text-neutral-500">
                <span className="rounded-xs bg-neutral-100 px-2 py-1">
                  +{stats.added} added
                </span>
                <span className="rounded-xs bg-neutral-100 px-2 py-1">
                  -{stats.removed} removed
                </span>
              </div>
              {preview.summary.length > 0 && (
                <ul className="mt-3 flex list-disc flex-col gap-1 pl-5 text-sm text-neutral-700">
                  {preview.summary.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              )}
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto bg-neutral-50 px-6 py-4">
              <div className="overflow-hidden rounded-xs border border-neutral-200 bg-white">
                <div className="grid grid-cols-[72px_72px_1fr] border-b border-neutral-200 bg-neutral-100 px-3 py-2 font-mono text-[11px] tracking-wide text-neutral-500 uppercase">
                  <span>Old</span>
                  <span>New</span>
                  <span>Diff</span>
                </div>
                <div className="font-mono text-xs">
                  {diffLines.map((line, index) => (
                    <div
                      key={`${line.type}-${index}-${line.text}`}
                      className={cn([
                        "grid grid-cols-[72px_72px_1fr] border-b border-neutral-100 px-3 py-1.5 last:border-b-0",
                        line.type === "added" && "bg-emerald-50",
                        line.type === "removed" && "bg-red-50",
                        line.type === "context" &&
                          "bg-neutral-50 text-neutral-400",
                      ])}
                    >
                      <span className="pr-3 text-right text-neutral-400">
                        {formatLineNumber(line.leftLine)}
                      </span>
                      <span className="pr-3 text-right text-neutral-400">
                        {formatLineNumber(line.rightLine)}
                      </span>
                      <span
                        className={cn([
                          "break-words whitespace-pre-wrap",
                          line.type === "added" && "text-emerald-800",
                          line.type === "removed" && "text-red-800",
                          line.type === "same" && "text-neutral-700",
                          line.type === "context" && "italic",
                        ])}
                      >
                        {line.type === "added"
                          ? `+ ${line.text}`
                          : line.type === "removed"
                            ? `- ${line.text}`
                            : line.type === "same"
                              ? `  ${line.text}`
                              : `… ${line.text} …`}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            <DialogFooter className="border-t border-neutral-200 px-6 py-4">
              <button
                type="button"
                onClick={() => onOpenChange(false)}
                className="rounded-xs px-3 py-2 text-sm text-neutral-600 transition-colors hover:bg-neutral-100"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={onApply}
                className="rounded-xs bg-neutral-900 px-3 py-2 text-sm text-white transition-colors hover:bg-neutral-800"
              >
                Apply Audit
              </button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
