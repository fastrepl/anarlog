import { Check, ChevronDown, ChevronUp, Code2, Copy } from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

export function GithubEmbed({
  code,
  fileName,
  url,
  startLine = 1,
  language: _language = "bash",
  highlightedHtml,
}: {
  code: string;
  fileName: string;
  url?: string;
  startLine?: number;
  language?: string;
  highlightedHtml?: string;
}) {
  const MAX_COLLAPSED_LINES = 10;
  const [copied, setCopied] = useState(false);
  const [tooltipOpen, setTooltipOpen] = useState(false);
  const [collapsedLines, setCollapsedLines] = useState<Set<number>>(new Set());
  const [isExpanded, setIsExpanded] = useState(false);

  const highlightedLines = useMemo(() => {
    if (!highlightedHtml) return null;
    const lineMatches = [
      ...highlightedHtml.matchAll(/<span class="line">(.*)<\/span>/g),
    ];
    return lineMatches.map((m) => m[1]);
  }, [highlightedHtml]);

  const lines = code.split("\n");

  if (lines[lines.length - 1] === "") {
    lines.pop();
  }

  const indentSize = useMemo(() => {
    for (const line of lines) {
      const match = line.match(/^( +)\S/);
      if (match) return match[1].length <= 4 ? match[1].length : 4;
    }
    return 4;
  }, [lines]);

  const indentLevels = useMemo(() => {
    const levels = lines.map((line) => {
      if (line.trim() === "") return -1;
      const match = line.match(/^( *)/);
      const spaces = match ? match[1].length : 0;
      return Math.floor(spaces / indentSize);
    });
    for (let i = 0; i < levels.length; i++) {
      if (levels[i] === -1) {
        levels[i] = i > 0 ? levels[i - 1] : 0;
      }
    }
    return levels;
  }, [lines, indentSize]);

  const foldRegions = useMemo(() => {
    const regions = new Map<number, number>();
    for (let i = 0; i < lines.length; i++) {
      if (i + 1 < lines.length && indentLevels[i + 1] > indentLevels[i]) {
        const baseLevel = indentLevels[i];
        let end = i + 1;
        while (end + 1 < lines.length && indentLevels[end + 1] > baseLevel) {
          end++;
        }
        regions.set(i, end);
      }
    }
    return regions;
  }, [lines, indentLevels]);

  const isLineVisible = useCallback(
    (index: number): boolean => {
      for (const [start, end] of foldRegions) {
        if (collapsedLines.has(start) && index > start && index <= end) {
          return false;
        }
      }
      return true;
    },
    [foldRegions, collapsedLines],
  );

  const toggleFold = useCallback((lineIndex: number) => {
    setCollapsedLines((prev) => {
      const next = new Set(prev);
      if (next.has(lineIndex)) next.delete(lineIndex);
      else next.add(lineIndex);
      return next;
    });
  }, []);

  const rawUrl = url
    ?.replace("github.com", "raw.githubusercontent.com")
    .replace("/blob/", "/")
    .replace(/#L\d+(-L\d+)?$/, "");

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTooltipOpen(true);
    setTimeout(() => {
      setCopied(false);
      setTooltipOpen(false);
    }, 2000);
  };

  const fileNameEl = url ? (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className="font-mono text-xs text-blue-600 hover:underline"
    >
      {fileName}
    </a>
  ) : (
    <span className="font-mono text-xs text-stone-600">{fileName}</span>
  );

  return (
    <TooltipProvider delayDuration={0}>
      <div className="my-4 overflow-hidden rounded-md border border-neutral-200 bg-stone-50">
        <div className="flex items-center justify-between border-b border-neutral-200 bg-stone-100 py-2 pr-2 pl-3">
          <div className="flex items-center gap-1.5">
            <Code2 className="h-4 w-4 shrink-0 text-stone-500" />
            {fileNameEl}
          </div>
          <div className="flex items-center gap-1">
            {rawUrl && (
              <a
                href={rawUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center rounded px-2 py-1 font-mono text-xs text-stone-600 transition-colors hover:bg-stone-200/80"
              >
                Raw
              </a>
            )}
            <Tooltip
              open={tooltipOpen}
              onOpenChange={(open) => {
                setTooltipOpen(open);
                if (!open) setCopied(false);
              }}
            >
              <TooltipTrigger asChild>
                <button
                  type="button"
                  onClick={handleCopy}
                  className="flex cursor-pointer items-center gap-1.5 rounded p-1 text-xs text-stone-600 transition-all hover:bg-stone-200/80"
                  aria-label={copied ? "Copied" : "Copy code"}
                >
                  {copied ? (
                    <Check className="h-3.5 w-3.5 text-green-600" />
                  ) : (
                    <Copy className="h-3.5 w-3.5" />
                  )}
                </button>
              </TooltipTrigger>
              <TooltipContent className="rounded-md bg-black text-white">
                {copied ? "Copied" : "Copy"}
              </TooltipContent>
            </Tooltip>
          </div>
        </div>
        <div className="overflow-x-auto bg-white">
          <table className="my-0! w-full border-collapse">
            <tbody>
              {lines.map((line, index) => {
                if (!isLineVisible(index)) return null;
                if (
                  !isExpanded &&
                  lines.length > MAX_COLLAPSED_LINES &&
                  index >= MAX_COLLAPSED_LINES
                )
                  return null;
                const isFoldable = foldRegions.has(index);
                const isFolded = collapsedLines.has(index);

                return (
                  <tr key={index} className="border-0 leading-5">
                    <td className="w-[1%] border-y-0 border-r border-neutral-200 bg-stone-50 py-0.5 pr-2 pl-2 font-mono text-sm whitespace-nowrap text-stone-400 select-none">
                      <div className="flex items-center justify-end">
                        <span className="flex w-4 shrink-0 items-center justify-center text-xs">
                          {isFoldable && (
                            <button
                              type="button"
                              onClick={() => toggleFold(index)}
                              className="cursor-pointer leading-none text-stone-400 hover:text-stone-600"
                            >
                              {isFolded ? "▸" : "▾"}
                            </button>
                          )}
                        </span>
                        <span className="text-right">{startLine + index}</span>
                      </div>
                    </td>
                    <td className="relative border-0 py-0.5 pr-4 font-mono text-sm whitespace-pre">
                      {Array.from({ length: indentLevels[index] }, (_, i) => (
                        <span
                          key={i}
                          className="absolute top-0 bottom-0 border-l border-neutral-200"
                          style={{
                            left: `calc(1rem + ${i * indentSize}ch + 1ch)`,
                          }}
                        />
                      ))}
                      {highlightedLines?.[index] != null ? (
                        <span
                          className="pl-4"
                          dangerouslySetInnerHTML={{
                            __html: highlightedLines[index] || " ",
                          }}
                        />
                      ) : (
                        <span className="pl-4 text-stone-700">
                          {line || " "}
                        </span>
                      )}
                      {isFolded && (
                        <span className="ml-2 rounded border border-stone-200 bg-stone-100 px-1.5 py-0.5 align-middle text-xs text-stone-400">
                          ⋯
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {lines.length > MAX_COLLAPSED_LINES && (
          <button
            type="button"
            onClick={() => setIsExpanded(!isExpanded)}
            className="flex w-full cursor-pointer items-center justify-center gap-1.5 border-t border-neutral-200 bg-stone-50 py-1.5 font-mono text-xs text-stone-500 transition-colors hover:text-stone-700"
          >
            {isExpanded ? (
              <>
                <ChevronUp className="h-3 w-3" />
                Collapse
              </>
            ) : (
              <>
                <ChevronDown className="h-3 w-3" />
                Expand ({lines.length - MAX_COLLAPSED_LINES} more lines)
              </>
            )}
          </button>
        )}
      </div>
    </TooltipProvider>
  );
}
