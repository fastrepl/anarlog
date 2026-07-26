import { PencilIcon } from "lucide-react";

import { defineTool } from "./define-tool";
import {
  MarkdownPreview,
  ToolCardBody,
  ToolCardFooterError,
  ToolCardFooters,
} from "./shared";

import { parseMcpObjectOutput } from "~/chat/mcp/mcp-output-parser";

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

export const ToolEditSummary = defineTool({
  icon: <PencilIcon />,
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
  renderFooter: ({ failed, errorText, parsed }) => (
    <ToolCardFooters failed={failed} errorText={errorText} rawText={null}>
      {parsed?.status === "error" ? (
        <div className="space-y-2">
          <ToolCardFooterError text={parsed.message ?? "Unknown error"} />
          {parsed.candidates && parsed.candidates.length > 0 ? (
            <div className="border-border bg-muted text-muted-foreground space-y-1 rounded-md border p-2 text-[12px]">
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
  ),
});
