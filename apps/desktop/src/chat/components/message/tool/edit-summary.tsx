import { PencilIcon } from "lucide-react";

import { defineTool } from "./define-tool";
import {
  MarkdownPreview,
  ToolCardBody,
  ToolCardFooterError,
  ToolCardFooters,
} from "./shared";

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
  if (output && typeof output === "object") {
    return output as EditSummaryOutput;
  }
  return null;
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
            <div className="space-y-1 rounded-md border border-neutral-200 bg-neutral-50 p-2 text-[12px] text-neutral-700">
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
