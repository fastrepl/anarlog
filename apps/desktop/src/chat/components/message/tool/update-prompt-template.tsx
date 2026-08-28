import { MagicWand } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";

import { defineTool } from "./define-tool";
import { ToolCardBody, ToolCardFooterError, ToolCardFooters } from "./shared";

import { parseMcpObjectOutput } from "~/chat/mcp/mcp-output-parser";

type UpdatePromptTemplateOutput = {
  status?: string;
  message?: string;
  lineCount?: number;
};

function parseUpdatePromptTemplateOutput(
  output: unknown,
): UpdatePromptTemplateOutput | null {
  return parseMcpObjectOutput<UpdatePromptTemplateOutput>(output);
}

export const ToolUpdatePromptTemplate = defineTool({
  icon: <MagicWand />,
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
        <pre {...stylex.props(styles.content)}>{input.content}</pre>
      </ToolCardBody>
    ) : null,
  renderFooter: ({ failed, errorText, parsed }) => (
    <ToolCardFooters failed={failed} errorText={errorText} rawText={null}>
      {parsed?.status === "error" ? (
        <ToolCardFooterError text={parsed.message ?? "Unknown error"} />
      ) : parsed?.message ? (
        <p {...stylex.props(styles.message)}>{parsed.message}</p>
      ) : null}
    </ToolCardFooters>
  ),
});

const styles = stylex.create({
  content: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    fontFamily: fonts.mono,
    fontSize: "0.6875rem",
    maxHeight: "12rem",
    overflow: "auto",
    padding: "0.75rem",
    whiteSpace: "pre-wrap",
  },
  message: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
});
