import type { AccountInfo } from "@hypr/plugin-auth";
import type { DeviceInfo } from "@hypr/plugin-misc";

import type { HyprUIMessage } from "../types";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export const CONTEXT_ENTITY_SOURCES = [
  "tool",
  "manual",
  "auto-current",
] as const;
export type ContextEntitySource = (typeof CONTEXT_ENTITY_SOURCES)[number];

export type ContextRef = {
  kind: "session";
  key: string;
  source?: ContextEntitySource;
  sessionId: string;
};

export type ContextEntity =
  | (ContextRef & {
      title?: string | null;
      date?: string | null;
      removable?: boolean;
    })
  | ({
      kind: "account";
      key: string;
      source?: ContextEntitySource;
    } & Partial<AccountInfo>)
  | ({
      kind: "device";
      key: string;
      source?: ContextEntitySource;
    } & Partial<DeviceInfo>);

export type ContextEntityKind = ContextEntity["kind"];

export function dedupeByKey<T extends { key: string }>(groups: T[][]): T[] {
  const seen = new Set<string>();
  const merged: T[] = [];
  for (const group of groups) {
    for (const item of group) {
      if (!seen.has(item.key)) {
        seen.add(item.key);
        merged.push(item);
      }
    }
  }
  return merged;
}

type ToolOutputAvailablePart = {
  type: string;
  state: "output-available";
  output?: unknown;
};

function isToolOutputAvailablePart(
  value: unknown,
): value is ToolOutputAvailablePart {
  return (
    isRecord(value) &&
    typeof value.type === "string" &&
    value.state === "output-available"
  );
}

function parseSearchSessionsOutput(output: unknown): ContextEntity[] {
  if (!isRecord(output) || !Array.isArray(output.results)) {
    return [];
  }

  return output.results.flatMap((item): ContextEntity[] => {
    if (
      !isRecord(item) ||
      (typeof item.id !== "string" && typeof item.id !== "number")
    ) {
      return [];
    }

    return [
      {
        kind: "session",
        key: `session:search:${item.id}`,
        source: "tool",
        sessionId: String(item.id),
        title: typeof item.title === "string" ? item.title : null,
      },
    ];
  });
}

const toolEntityExtractors: Record<
  string,
  (output: unknown) => ContextEntity[]
> = {
  search_sessions: parseSearchSessionsOutput,
};

export function extractToolContextEntities(
  messages: Array<Pick<HyprUIMessage, "parts">>,
): ContextEntity[] {
  const seen = new Set<string>();
  const entities: ContextEntity[] = [];

  for (const message of messages) {
    if (!Array.isArray(message.parts)) continue;
    for (const part of message.parts) {
      if (!isToolOutputAvailablePart(part) || !part.type.startsWith("tool-")) {
        continue;
      }

      const toolName = part.type.slice(5);
      const extractor = toolEntityExtractors[toolName];
      if (!extractor) continue;

      for (const entity of extractor(part.output)) {
        if (!seen.has(entity.key)) {
          seen.add(entity.key);
          entities.push(entity);
        }
      }
    }
  }

  return entities;
}
