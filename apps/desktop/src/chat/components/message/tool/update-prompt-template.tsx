import { WandSparklesIcon } from "lucide-react";

import { defineTool } from "./define-tool";
import { ToolCardBody, ToolCardFooterError, ToolCardFooters } from "./shared";

type UpdatePromptTemplateOutput = {
  status?: string;
  message?: string;
  lineCount?: number;
};

function parseUpdatePromptTemplateOutput(
  output: unknown,
): UpdatePromptTemplateOutput | null {
  if (output && typeof output === "object") {
    return output as UpdatePromptTemplateOutput;
  }

  return null;
}

export const ToolUpdatePromptTemplate = defineTool({
  icon: <WandSparklesIcon />,
  parseFn: parseUpdatePromptTemplateOutput,
  isDone: (parsed) => parsed?.status === "applied",
  label: ({ running, failed, parsed }) => {
    if (running) return "Updating prompt draft";
    if (failed) return "Prompt update failed";
    if (parsed?.status === "applied") return "Prompt draft updated";
    return "Update prompt draft";
  },
  renderBody: (input) =>
    typeof input?.content === "string" ? (
      <ToolCardBody>
        <pre className="max-h-48 overflow-auto rounded-md border border-neutral-200 bg-neutral-50 p-3 font-mono text-[11px] whitespace-pre-wrap text-neutral-700">
          {input.content}
        </pre>
      </ToolCardBody>
    ) : null,
  renderFooter: ({ failed, errorText, parsed }) => (
    <ToolCardFooters failed={failed} errorText={errorText} rawText={null}>
      {parsed?.status === "error" ? (
        <ToolCardFooterError text={parsed.message ?? "Unknown error"} />
      ) : parsed?.message ? (
        <p className="text-xs text-neutral-600">{parsed.message}</p>
      ) : null}
    </ToolCardFooters>
  ),
});
