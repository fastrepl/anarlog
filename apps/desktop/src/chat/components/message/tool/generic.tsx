import { Wrench } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

import { useToolState } from "./shared";

import { Disclosure } from "~/chat/components/message/shared";
import { extractMcpOutputText } from "~/chat/mcp/mcp-output-parser";
import { CONTEXT_TEXT_FIELD } from "~/chat/tools/context-text";

function formatToolName(name: string): string {
  return name.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());
}

function formatOutputText(output: unknown): string | null {
  const mcpText = extractMcpOutputText(output);
  if (mcpText) {
    return mcpText;
  }

  if (typeof output === "string") {
    return output;
  }

  if (output === null || output === undefined) {
    return null;
  }

  try {
    if (
      typeof output === "object" &&
      output !== null &&
      CONTEXT_TEXT_FIELD in output
    ) {
      const { [CONTEXT_TEXT_FIELD]: _contextText, ...rest } = output as Record<
        string,
        unknown
      >;
      return JSON.stringify(rest, null, 2);
    }

    return JSON.stringify(output, null, 2);
  } catch {
    return String(output);
  }
}

export function ToolGeneric({ part }: { part: Record<string, unknown> }) {
  const toolName = String(
    part.toolName ??
      (typeof part.type === "string" ? part.type.replace("tool-", "") : "tool"),
  );
  const { failed } = useToolState(part as { state: string });
  const done = (part.state as string) === "output-available";

  if (done || failed) {
    const outputText = done ? formatOutputText(part.output) : null;

    return (
      <Disclosure
        icon={<Wrench {...stylex.props(styles.icon)} />}
        title={
          failed
            ? `${formatToolName(toolName)} failed`
            : formatToolName(toolName)
        }
      >
        <div {...stylex.props(styles.content)}>
          <InputDisplay input={part.input} />
          {failed ? (
            <p {...stylex.props(styles.error)}>
              {String(part.errorText ?? "Unknown error")}
            </p>
          ) : null}
          {outputText ? (
            <p {...stylex.props(styles.output)}>{outputText}</p>
          ) : null}
        </div>
      </Disclosure>
    );
  }

  return (
    <Disclosure
      icon={<Wrench {...stylex.props(styles.icon)} />}
      title={`Running ${formatToolName(toolName)}…`}
      disabled
    >
      {null}
    </Disclosure>
  );
}

function InputDisplay({ input }: { input: unknown }) {
  if (!input || typeof input !== "object") return null;
  const entries = Object.entries(input as Record<string, unknown>);
  if (entries.length === 0) return null;

  return (
    <dl {...stylex.props(styles.input)}>
      {entries.map(([key, value]) => (
        <div key={key}>
          <dt {...stylex.props(styles.inputTerm)}>{key}: </dt>
          <dd {...stylex.props(styles.inputDescription)}>
            {typeof value === "string" ? value : JSON.stringify(value)}
          </dd>
        </div>
      ))}
    </dl>
  );
}

const styles = stylex.create({
  icon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  error: {
    color: "oklch(63.7% 0.237 25.331)",
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  output: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    whiteSpace: "pre-wrap",
  },
  input: {
    color: colors.mutedForeground,
    display: "flex",
    flexDirection: "column",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
  },
  inputTerm: {
    color: colors.mutedForeground,
    display: "inline",
    fontWeight: 500,
  },
  inputDescription: {
    display: "inline",
    overflowWrap: "break-word",
    whiteSpace: "pre-wrap",
  },
});
