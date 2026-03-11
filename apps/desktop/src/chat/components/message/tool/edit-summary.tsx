import { PencilIcon } from "lucide-react";

import {
  MarkdownPreview,
  ToolCard,
  ToolCardBody,
  ToolCardFooterError,
  ToolCardFooters,
  ToolCardHeader,
  useToolState,
} from "./shared";

import type { ToolRenderer } from "~/chat/components/message/types";

type Renderer = ToolRenderer<"tool-edit_summary">;

function headerLabel(
  running: boolean,
  failed: boolean,
  status: string | undefined,
): string {
  if (running) return "Edit summary — review tab opened";
  if (failed) return "Summary edit failed";
  if (status === "applied") return "Summary updated";
  if (status === "declined") return "Summary edit declined";
  return "Edit summary";
}

export const ToolEditSummary: Renderer = ({ part }) => {
  const { running, failed, done } = useToolState(part);

  const output =
    done && part.output && typeof part.output === "object"
      ? (part.output as {
          status?: string;
          message?: string;
          candidates?: Array<{
            enhancedNoteId: string;
            title: string;
            templateId?: string;
            position?: number;
          }>;
        })
      : null;

  return (
    <ToolCard failed={failed}>
      <ToolCardHeader
        icon={<PencilIcon />}
        running={running}
        failed={failed}
        done={done && output?.status === "applied"}
        label={headerLabel(running, failed, output?.status)}
      />

      {part.input?.content ? (
        <ToolCardBody>
          <MarkdownPreview>{part.input.content}</MarkdownPreview>
        </ToolCardBody>
      ) : null}

      <ToolCardFooters
        failed={failed}
        errorText={part.errorText}
        rawText={null}
      >
        {output?.status === "error" ? (
          <div className="space-y-2">
            <ToolCardFooterError text={output.message ?? "Unknown error"} />
            {output.candidates && output.candidates.length > 0 ? (
              <div className="space-y-1 rounded-md border border-neutral-200 bg-neutral-50 p-2 text-[12px] text-neutral-700">
                {output.candidates.map((candidate) => (
                  <div key={candidate.enhancedNoteId}>
                    {candidate.title} ({candidate.enhancedNoteId})
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}
      </ToolCardFooters>
    </ToolCard>
  );
};
