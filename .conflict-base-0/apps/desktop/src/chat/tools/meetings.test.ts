import { asSchema } from "ai";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listMeetings: vi.fn(),
  getMeeting: vi.fn(),
  getMeetingTranscript: vi.fn(),
  getRecurringMeetingHistory: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => mocks);

import {
  buildGetMeetingTool,
  buildGetMeetingTranscriptTool,
  buildGetRecurringMeetingHistoryTool,
  buildListMeetingsTool,
} from "./meetings";

describe("canonical meeting chat tools", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("passes MCP-shaped inputs to the shared meeting service", async () => {
    mocks.listMeetings.mockResolvedValue({ meetings: [], pagination: {} });
    mocks.getMeeting.mockResolvedValue({ id: "meeting-1" });
    mocks.getMeetingTranscript.mockResolvedValue({
      meeting_id: "meeting-1",
      words: [],
      pagination: {},
    });
    mocks.getRecurringMeetingHistory.mockResolvedValue({
      meetings: [],
      pagination: {},
    });

    await (buildListMeetingsTool() as any).execute({
      query: "planning",
      limit: 10,
      offset: 20,
    });
    await (buildGetMeetingTool() as any).execute({
      meeting_id: "meeting-1",
    });
    await (buildGetMeetingTranscriptTool() as any).execute({
      meeting_id: "meeting-1",
      limit: 200,
      offset: 0,
    });
    await (buildGetRecurringMeetingHistoryTool() as any).execute({
      meeting_id: "meeting-1",
      limit: 20,
      offset: 0,
    });

    expect(mocks.listMeetings).toHaveBeenCalledWith({
      query: "planning",
      limit: 10,
      offset: 20,
    });
    expect(mocks.getMeeting).toHaveBeenCalledWith({
      meeting_id: "meeting-1",
    });
    expect(mocks.getMeetingTranscript).toHaveBeenCalledWith({
      meeting_id: "meeting-1",
      limit: 200,
      offset: 0,
    });
    expect(mocks.getRecurringMeetingHistory).toHaveBeenCalledWith({
      meeting_id: "meeting-1",
      limit: 20,
      offset: 0,
    });
  });

  it("matches the canonical MCP tool names, descriptions, and input schemas", async () => {
    const repositoryRoot = existsSync(
      resolve(process.cwd(), "../cli/Cargo.toml"),
    )
      ? resolve(process.cwd(), "../..")
      : process.cwd();
    const snapshot = readFileSync(
      resolve(
        repositoryRoot,
        "apps/cli/src/snapshots/anarlog_cli__mcp__tests__mcp_contract.snap",
      ),
      "utf8",
    );
    const contract = JSON.parse(
      snapshot.slice(snapshot.indexOf("\n---\n") + 5),
    ) as {
      tools: Array<{
        name: string;
        description: string;
        inputSchema: Record<string, unknown>;
      }>;
    };
    const chatTools = {
      list_meetings: buildListMeetingsTool(),
      get_meeting: buildGetMeetingTool(),
      get_meeting_transcript: buildGetMeetingTranscriptTool(),
      get_recurring_meeting_history: buildGetRecurringMeetingHistoryTool(),
    };

    expect(Object.keys(chatTools).sort()).toEqual(
      contract.tools.map((tool) => tool.name).sort(),
    );

    for (const [name, chatTool] of Object.entries(chatTools)) {
      const canonical = contract.tools.find((tool) => tool.name === name);
      expect(canonical).toBeDefined();
      expect(chatTool.description).toBe(canonical?.description);
      const chatSchema = await asSchema(
        chatTool.inputSchema as Parameters<typeof asSchema>[0],
      ).jsonSchema;
      expect(normalizeSchema(chatSchema)).toEqual(
        normalizeSchema(canonical?.inputSchema ?? {}),
      );
    }
  });
});

function normalizeSchema(schema: unknown) {
  const objectSchema = schema as Record<string, unknown>;
  const properties = Object.fromEntries(
    Object.entries(
      (objectSchema.properties as Record<string, Record<string, unknown>>) ??
        {},
    ).map(([name, property]) => [
      name,
      compact({
        type: Array.isArray(property.type)
          ? property.type.find((type) => type !== "null")
          : property.type,
        description: property.description,
        minimum: property.minimum,
        maximum:
          property.maximum === Number.MAX_SAFE_INTEGER
            ? undefined
            : property.maximum,
      }),
    ]),
  );

  return {
    properties,
    required: [
      ...((objectSchema.required as string[] | undefined) ?? []),
    ].sort(),
  };
}

function compact(value: Record<string, unknown>) {
  return Object.fromEntries(
    Object.entries(value).filter(([, entry]) => entry !== undefined),
  );
}
