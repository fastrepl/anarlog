import { CircleDotIcon, SearchIcon } from "lucide-react";

import { cn } from "@hypr/utils";

import { defineTool } from "./define-tool";
import { ToolCardBody } from "./shared";

import { parseSearchIssuesOutput } from "~/chat/mcp/support-mcp-tools";

export const ToolSearchIssues = defineTool({
  icon: <SearchIcon />,
  parseFn: parseSearchIssuesOutput,
  label: ({ running, failed, parsed, input }) => {
    const query = input?.query ?? "";
    if (running) return `Searching issues: ${query}`;
    if (failed) return `Issue search failed: ${query}`;
    if (parsed)
      return `${parsed.total_results} issue${parsed.total_results === 1 ? "" : "s"} found`;
    return "Search issues";
  },
  renderBody: (_, parsed) => {
    if (!parsed) return null;
    if (parsed.issues.length === 0) {
      return (
        <ToolCardBody>
          <p className="py-1 text-center text-xs text-neutral-500">
            No issues found
          </p>
        </ToolCardBody>
      );
    }
    return (
      <ToolCardBody>
        {parsed.issues.map((issue) => (
          <a
            key={issue.url}
            href={issue.url}
            target="_blank"
            rel="noreferrer"
            className="flex items-start gap-2 rounded-md border border-neutral-200 p-2 transition-colors hover:border-neutral-300"
          >
            <CircleDotIcon
              className={cn([
                "mt-0.5 h-3.5 w-3.5 shrink-0",
                issue.state.toLowerCase() === "open"
                  ? "text-emerald-500"
                  : "text-purple-500",
              ])}
            />
            <div className="flex min-w-0 flex-col gap-0.5">
              <p className="text-xs leading-snug text-neutral-800">
                {issue.title}
              </p>
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-neutral-500">
                  #{issue.number}
                </span>
                {issue.labels.map((label) => (
                  <span
                    key={label}
                    className="inline-flex items-center rounded-full border border-neutral-200 bg-neutral-100 px-1.5 py-0 text-[10px] text-neutral-500"
                  >
                    {label}
                  </span>
                ))}
              </div>
            </div>
          </a>
        ))}
      </ToolCardBody>
    );
  },
});
