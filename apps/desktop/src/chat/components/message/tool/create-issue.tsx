import { CircleDotIcon, TagIcon } from "lucide-react";

import {
  MarkdownPreview,
  ToolCard,
  ToolCardApproval,
  ToolCardBody,
  ToolCardFooters,
  ToolCardFooterSuccess,
  ToolCardHeader,
  useMcpOutput,
  useToolApproval,
  useToolState,
} from "./shared";

import type { ToolRenderer } from "~/chat/components/message/types";
import { parseCreateIssueOutput } from "~/chat/mcp/support-mcp-tools";

type Renderer = ToolRenderer<"tool-create_issue">;

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

function headerLabel(
  running: boolean,
  awaitingApproval: boolean,
  failed: boolean,
  parsed: ReturnType<typeof parseCreateIssueOutput>,
): string {
  if (awaitingApproval) return "Create issue — review needed";
  if (running) return "Drafting GitHub issue...";
  if (failed) return "Issue creation failed";
  if (parsed) return `Created #${parsed.issue_number}`;
  return "GitHub Issue";
}

export const ToolCreateIssue: Renderer = ({ part }) => {
  const { running, failed, done } = useToolState(part);
  const { parsed, rawText } = useMcpOutput(
    done,
    part.output,
    parseCreateIssueOutput,
  );
  const labels = normalizeLabels(part.input?.labels);
  const awaitingApproval = useToolApproval(running);

  return (
    <ToolCard failed={failed}>
      <ToolCardHeader
        icon={<CircleDotIcon />}
        running={running}
        awaitingApproval={awaitingApproval}
        failed={failed}
        done={!!parsed}
        label={headerLabel(running, awaitingApproval, failed, parsed)}
      />

      {part.input ? (
        <ToolCardBody>
          {part.input.title ? (
            <p className="text-sm leading-snug font-semibold text-neutral-900">
              {part.input.title}
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
          {part.input.body ? (
            <MarkdownPreview>{part.input.body}</MarkdownPreview>
          ) : null}
        </ToolCardBody>
      ) : null}

      {awaitingApproval ? <ToolCardApproval /> : null}

      <ToolCardFooters
        failed={failed}
        errorText={part.errorText}
        rawText={rawText}
      >
        {parsed ? (
          <ToolCardFooterSuccess
            href={parsed.issue_url}
            label={`Issue #${parsed.issue_number} created`}
          />
        ) : null}
      </ToolCardFooters>
    </ToolCard>
  );
};
