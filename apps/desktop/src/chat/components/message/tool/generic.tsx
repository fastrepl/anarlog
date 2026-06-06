import { WrenchIcon } from "lucide-react";

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
        icon={<WrenchIcon className="h-3 w-3" />}
        title={
          failed
            ? `${formatToolName(toolName)} failed`
            : formatToolName(toolName)
        }
      >
        <div className="flex flex-col gap-2">
          <InputDisplay input={part.input} />
          {failed ? (
            <p className="text-xs text-red-500">
              {String(part.errorText ?? "Unknown error")}
            </p>
          ) : null}
          {outputText ? (
            <p className="text-muted-foreground text-xs whitespace-pre-wrap">
              {outputText}
            </p>
          ) : null}
        </div>
      </Disclosure>
    );
  }

  return (
    <Disclosure
      icon={<WrenchIcon className="h-3 w-3" />}
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
    <dl className="text-muted-foreground flex flex-col gap-1 text-xs">
      {entries.map(([key, value]) => (
        <div key={key}>
          <dt className="text-muted-foreground inline font-medium">{key}: </dt>
          <dd className="inline wrap-break-word whitespace-pre-wrap">
            {typeof value === "string" ? value : JSON.stringify(value)}
          </dd>
        </div>
      ))}
    </dl>
  );
}
