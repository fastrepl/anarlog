import type { AccountInfo } from "@anlg/plugin-auth";
import type { DeviceInfo } from "@anlg/plugin-misc";

import type { AnlgUIMessage } from "../types";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export const CONTEXT_ENTITY_SOURCES = [
  "tool",
  "manual",
  "auto-current",
] as const;
export type ContextEntitySource = (typeof CONTEXT_ENTITY_SOURCES)[number];

type BaseContextRef = {
  key: string;
  source?: ContextEntitySource;
};

export type SessionContextRef = BaseContextRef & {
  kind: "session";
  sessionId: string;
};

export type HumanContextRef = BaseContextRef & {
  kind: "human";
  humanId: string;
};

export type OrganizationContextRef = BaseContextRef & {
  kind: "organization";
  organizationId: string;
};

export type FolderContextRef = BaseContextRef & {
  kind: "folder";
  folderId: string;
};

export type ContextRef =
  | SessionContextRef
  | HumanContextRef
  | OrganizationContextRef
  | FolderContextRef;

export type CalendarEventContextRef = BaseContextRef & {
  kind: "calendar_event";
  eventId: string;
  linkedSessionId?: string | null;
};

export type ContextEntity =
  | (SessionContextRef & {
      title?: string | null;
      date?: string | null;
      removable?: boolean;
    })
  | (HumanContextRef & {
      name?: string | null;
      email?: string | null;
      organizationName?: string | null;
      removable?: boolean;
    })
  | (OrganizationContextRef & {
      name?: string | null;
      removable?: boolean;
    })
  | (FolderContextRef & {
      title?: string | null;
      removable?: boolean;
    })
  | (CalendarEventContextRef & {
      title?: string | null;
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

function resultItems(output: unknown): Record<string, unknown>[] {
  if (!isRecord(output)) {
    return [];
  }

  const results = Array.isArray(output.results)
    ? output.results
    : Array.isArray(output.meetings)
      ? output.meetings
      : [];

  return results.filter(isRecord);
}

function itemId(item: Record<string, unknown>): string | null {
  if (
    typeof item.meeting_id === "string" ||
    typeof item.meeting_id === "number"
  ) {
    return String(item.meeting_id);
  }
  if (typeof item.id === "string" || typeof item.id === "number") {
    return String(item.id);
  }
  return null;
}

function parseSearchMeetingsOutput(output: unknown): ContextEntity[] {
  return resultItems(output).flatMap((item): ContextEntity[] => {
    const sessionId = itemId(item);
    if (!sessionId) {
      return [];
    }

    return [
      {
        kind: "session",
        key: `session:search:${sessionId}`,
        source: "tool",
        sessionId,
        title: typeof item.title === "string" ? item.title : null,
      },
    ];
  });
}

function parseSearchContactsOutput(output: unknown): ContextEntity[] {
  return resultItems(output).flatMap((item): ContextEntity[] => {
    if (typeof item.id !== "string" && typeof item.id !== "number") {
      return [];
    }

    const humanId = String(item.id);
    return [
      {
        kind: "human",
        key: `human:search:${humanId}`,
        source: "tool",
        humanId,
        name: typeof item.name === "string" ? item.name : null,
        email: typeof item.email === "string" ? item.email : null,
        organizationName:
          typeof item.organization === "string" ? item.organization : null,
      },
    ];
  });
}

function parseSearchCalendarEventsOutput(output: unknown): ContextEntity[] {
  return resultItems(output).flatMap((item): ContextEntity[] => {
    if (typeof item.id !== "string" && typeof item.id !== "number") {
      return [];
    }

    const eventId = String(item.id);
    const linkedSessionId =
      typeof item.linkedSessionId === "string" ? item.linkedSessionId : null;

    return [
      {
        kind: "calendar_event",
        key: `calendar_event:search:${eventId}`,
        source: "tool",
        eventId,
        linkedSessionId,
        title: typeof item.title === "string" ? item.title : null,
      },
    ];
  });
}

const toolEntityExtractors: Record<
  string,
  (output: unknown) => ContextEntity[]
> = {
  list_meetings: parseSearchMeetingsOutput,
  search_meetings: parseSearchMeetingsOutput,
  search_sessions: parseSearchMeetingsOutput,
  search_meeting_content: parseSearchMeetingsOutput,
  find_related_meetings: parseSearchMeetingsOutput,
  search_contacts: parseSearchContactsOutput,
  search_calendar_events: parseSearchCalendarEventsOutput,
};

export function extractToolContextEntities(
  messages: Array<Pick<AnlgUIMessage, "parts">>,
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
