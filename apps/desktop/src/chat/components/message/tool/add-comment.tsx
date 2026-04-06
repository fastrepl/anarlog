import { MessageSquareIcon } from "lucide-react";

import { defineTool } from "./define-tool";
import { MarkdownPreview, ToolCardBody, ToolCardFooterSuccess } from "./shared";

import { parseAddCommentOutput } from "~/chat/mcp/support-mcp-tools";

export const ToolAddComment = defineTool({
  icon: <MessageSquareIcon />,
  approval: true,
  parseFn: parseAddCommentOutput,
  label: ({ running, awaitingApproval, failed, parsed, input }) => {
    const n = input?.issue_number ?? "?";
    if (awaitingApproval) return `Add comment to #${n} — review needed`;
    if (running) return `Commenting on #${n}...`;
    if (failed) return `Comment failed for #${n}`;
    if (parsed) return `Comment posted to #${n}`;
    return `Comment on #${n}`;
  },
  renderBody: (input) =>
    input ? (
      <ToolCardBody>
        <p className="text-xs font-medium text-neutral-600">
          Issue #{input.issue_number}
        </p>
        {input.body ? <MarkdownPreview>{input.body}</MarkdownPreview> : null}
      </ToolCardBody>
    ) : null,
  renderSuccess: (parsed) => (
    <ToolCardFooterSuccess href={parsed.comment_url} label="Comment posted" />
  ),
});
