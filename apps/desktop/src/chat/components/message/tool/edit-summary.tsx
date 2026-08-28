import { Pencil } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

import { defineTool } from "./define-tool";
import {
  MarkdownPreview,
  ToolCardBody,
  ToolCardFooterError,
  ToolCardFooters,
} from "./shared";

import { parseMcpObjectOutput } from "~/chat/mcp/mcp-output-parser";
import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import {
  applyProposalReview,
  declineProposalReview,
} from "~/session/proposal-review";

type EditSummaryOutput = {
  status?: string;
  message?: string;
  candidates?: Array<{
    enhancedNoteId: string;
    title: string;
    templateId?: string;
    position?: number;
  }>;
};

function parseEditSummaryOutput(output: unknown): EditSummaryOutput | null {
  return parseMcpObjectOutput<EditSummaryOutput>(output);
}

function EditActions({
  toolCallId,
  target,
}: {
  toolCallId: string;
  target: "memo" | "summary";
}) {
  const editPending = usePendingEditStore((state) =>
    state.edits.has(toolCallId),
  );

  if (!editPending) {
    return null;
  }

  return (
    <div {...stylex.props(styles.editActions)}>
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => void declineProposalReview(toolCallId)}
      >
        Decline
      </Button>
      <Button
        type="button"
        size="sm"
        onClick={() => void applyProposalReview(toolCallId)}
      >
        Apply to {target}
      </Button>
    </div>
  );
}

export const ToolEditSummary = defineTool({
  icon: <Pencil />,
  parseFn: parseEditSummaryOutput,
  isDone: (parsed) => parsed?.status === "applied",
  label: ({ running, failed, parsed }) => {
    if (running) return "Edit summary — review tab opened";
    if (failed) return "Summary edit failed";
    if (parsed?.status === "applied") return "Summary updated";
    if (parsed?.status === "declined") return "Summary edit declined";
    return "Edit summary";
  },
  renderBody: (input) =>
    input?.content ? (
      <ToolCardBody>
        <MarkdownPreview>{input.content}</MarkdownPreview>
      </ToolCardBody>
    ) : null,
  renderFooter: ({ failed, errorText, parsed, toolCallId }) => (
    <>
      <ToolCardFooters failed={failed} errorText={errorText} rawText={null}>
        {parsed?.status === "error" ? (
          <div {...stylex.props(styles.errorContent)}>
            <ToolCardFooterError text={parsed.message ?? "Unknown error"} />
            {parsed.candidates && parsed.candidates.length > 0 ? (
              <div {...stylex.props(styles.candidates)}>
                {parsed.candidates.map((candidate) => (
                  <div key={candidate.enhancedNoteId}>
                    {candidate.title} ({candidate.enhancedNoteId})
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}
      </ToolCardFooters>
      <EditActions toolCallId={toolCallId} target="summary" />
    </>
  ),
});

const styles = stylex.create({
  editActions: {
    borderColor: `color-mix(in oklab, ${colors.border} 80%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "flex-end",
    paddingBlock: "0.625rem",
    paddingInline: "0.875rem",
  },
  errorContent: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  candidates: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    flexDirection: "column",
    fontSize: "0.75rem",
    gap: "0.25rem",
    padding: "0.5rem",
  },
});

export const ToolEditMemo = defineTool({
  icon: <Pencil />,
  parseFn: parseEditSummaryOutput,
  isDone: (parsed) => parsed?.status === "applied",
  label: ({ running, failed, parsed }) => {
    if (running) return "Edit memo — review tab opened";
    if (failed) return "Memo edit failed";
    if (parsed?.status === "applied") return "Memo updated";
    if (parsed?.status === "declined") return "Memo edit declined";
    return "Edit memo";
  },
  renderBody: (input) =>
    input?.content ? (
      <ToolCardBody>
        <MarkdownPreview>{input.content}</MarkdownPreview>
      </ToolCardBody>
    ) : null,
  renderFooter: ({ failed, errorText, parsed, toolCallId }) => (
    <>
      <ToolCardFooters failed={failed} errorText={errorText} rawText={null}>
        {parsed?.status === "error" ? (
          <ToolCardFooterError text={parsed.message ?? "Unknown error"} />
        ) : null}
      </ToolCardFooters>
      <EditActions toolCallId={toolCallId} target="memo" />
    </>
  ),
});
