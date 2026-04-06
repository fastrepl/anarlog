import { CircleDotIcon, TagIcon } from "lucide-react";

import { defineTool } from "./define-tool";
import { MarkdownPreview, ToolCardBody, ToolCardFooterSuccess } from "./shared";

import { parseCreateIssueOutput } from "~/chat/mcp/support-mcp-tools";

function normalizeLabels(
  labels: ReadonlyArray<string | undefined> | null | undefined,
): string[] {
  return (
    labels?.filter(
      (label): label is string =>
        typeof label === "string" && label.trim().length > 0,
    ) ?? []
  );
}

export const ToolCreateIssue = defineTool({
  icon: <CircleDotIcon />,
  approval: true,
  parseFn: parseCreateIssueOutput,
  label: ({ running, awaitingApproval, failed, parsed }) => {
    if (awaitingApproval) return "Create issue — review needed";
    if (running) return "Drafting GitHub issue...";
    if (failed) return "Issue creation failed";
    if (parsed) return `Created #${parsed.issue_number}`;
    return "GitHub Issue";
  },
  renderBody: (input) => {
    if (!input) return null;
    const labels = normalizeLabels(input.labels);
    return (
      <ToolCardBody>
        {input.title ? (
          <p className="text-sm leading-snug font-semibold text-neutral-900">
            {input.title}
          </p>
        ) : null}
        {labels.length > 0 ? (
          <div className="flex flex-wrap items-center gap-1.5">
            <TagIcon className="h-3 w-3 shrink-0 text-neutral-400" />
            {labels.map((label) => (
              <span
                key={label}
                className="inline-flex items-center rounded-full border border-neutral-200 bg-neutral-100 px-2 py-0.5 text-[11px] font-medium text-neutral-600"
              >
                {label}
              </span>
            ))}
          </div>
        ) : null}
        {input.body ? <MarkdownPreview>{input.body}</MarkdownPreview> : null}
      </ToolCardBody>
    );
  },
  renderSuccess: (parsed) => (
    <ToolCardFooterSuccess
      href={parsed.issue_url}
      label={`Issue #${parsed.issue_number} created`}
    />
  ),
});
