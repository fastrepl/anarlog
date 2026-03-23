function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export type McpTextContentOutput = {
  content: Array<{
    type: string;
    text?: string;
  }>;
};

export function extractMcpOutputText(output: unknown): string | null {
  if (!isRecord(output) || !Array.isArray(output.content)) {
    return null;
  }

  const text = output.content
    .filter(
      (item): item is { type: string; text: string } =>
        isRecord(item) && item.type === "text" && typeof item.text === "string",
    )
    .map((item) => item.text)
    .join("\n");

  return text || null;
}

export function readMcpJsonText(output: unknown): unknown {
  const text = extractMcpOutputText(output);
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

export function parseMcpToolOutput<T>(
  output: unknown,
  guard: (value: unknown) => value is T,
): T | null {
  const value = readMcpJsonText(output);
  return guard(value) ? value : null;
}
