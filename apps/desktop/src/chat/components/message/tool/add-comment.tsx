import { MessageSquareIcon } from "lucide-react";

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
import { parseAddCommentOutput } from "~/chat/mcp/support-mcp-tools";

type Renderer = ToolRenderer<"tool-add_comment">;

function headerLabel(
  running: boolean,
  awaitingApproval: boolean,
  failed: boolean,
  issueNumber: string | number,
  parsed: ReturnType<typeof parseAddCommentOutput>,
): string {
  if (awaitingApproval) return `Add comment to #${issueNumber} — review needed`;
  if (running) return `Commenting on #${issueNumber}...`;
  if (failed) return `Comment failed for #${issueNumber}`;
  if (parsed) return `Comment posted to #${issueNumber}`;
  return `Comment on #${issueNumber}`;
}

export const ToolAddComment: Renderer = ({ part }) => {
  const { running, failed, done } = useToolState(part);
  const { parsed, rawText } = useMcpOutput(
    done,
    part.output,
    parseAddCommentOutput,
  );
  const issueNumber = part.input?.issue_number ?? "?";
  const awaitingApproval = useToolApproval(running);

  return (
    <ToolCard failed={failed}>
      <ToolCardHeader
        icon={<MessageSquareIcon />}
        running={running}
        awaitingApproval={awaitingApproval}
        failed={failed}
        done={!!parsed}
        label={headerLabel(
          running,
          awaitingApproval,
          failed,
          issueNumber,
          parsed,
        )}
      />

      {part.input ? (
        <ToolCardBody>
          <p className="text-xs font-medium text-neutral-600">
            Issue #{part.input.issue_number}
          </p>
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
            href={parsed.comment_url}
            label="Comment posted"
          />
        ) : null}
      </ToolCardFooters>
    </ToolCard>
  );
};
