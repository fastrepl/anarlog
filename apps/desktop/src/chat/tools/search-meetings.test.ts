import { beforeEach, describe, expect, it, vi } from "vitest";

import { searchMeetingContent } from "./note-files";
import { buildSearchMeetingsTool } from "./search-meetings";

import { loadSessionSummariesByFolder } from "~/session/queries";

vi.mock("./note-files", () => ({
  searchMeetingContent: vi.fn(),
}));

vi.mock("~/session/queries", () => ({
  loadSessionSummariesByFolder: vi.fn(),
}));

describe("search meetings chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("keeps full-content search results behind meeting vocabulary", async () => {
    const search = vi.fn().mockResolvedValue([
      {
        score: 0.9,
        document: {
          id: "human-1",
          type: "human",
          title: "Ada",
          content: "contract renewal",
          created_at: 100,
        },
      },
      {
        score: 0.8,
        document: {
          id: "meeting-1",
          type: "session",
          title: "Customer call",
          content: "Discussed contract renewal timing and next steps.",
          created_at: 200,
        },
      },
    ]);
    const meetingSearchTool = buildSearchMeetingsTool({ search } as any);

    const result = await (meetingSearchTool as any).execute({
      query: "contract renewal",
      filters: {
        created_at: { kind: "absolute", gte: 100, lte: 300 },
      },
      limit: 1,
    });

    expect(search).toHaveBeenCalledWith("contract renewal", {
      created_at: {
        gte: 100,
        lte: 300,
        gt: undefined,
        lt: undefined,
        eq: undefined,
      },
    });
    expect(result).toEqual({
      results: [
        {
          id: "meeting-1",
          title: "Customer call",
          excerpt: "Discussed contract renewal timing and next steps.",
          score: 0.8,
          created_at: 200,
        },
      ],
    });
  });

  it("searches only the active folder instead of post-filtering global hits", async () => {
    const search = vi.fn();
    const meetingSearchTool = buildSearchMeetingsTool({
      search,
      getFolderFilter: () => "CS 101",
    } as any);

    vi.mocked(loadSessionSummariesByFolder).mockResolvedValue([
      {
        id: "cs-101",
        title: "CS 101 lecture",
        created_at: "2026-08-01T00:00:00.000Z",
        event_json: "",
      },
    ]);
    vi.mocked(searchMeetingContent).mockResolvedValue({
      query: "midterm",
      scanned: 1,
      results: [
        {
          sessionId: "cs-101",
          title: "CS 101 lecture",
          date: "2026-08-01T00:00:00.000Z",
          score: 0.8,
          snippets: [{ section: "Raw note", text: "midterm review" }],
        },
      ],
    });

    const result = await (meetingSearchTool as any).execute({
      query: "midterm",
    });

    expect(search).not.toHaveBeenCalled();
    expect(searchMeetingContent).toHaveBeenCalledWith({
      query: "midterm",
      sessionIds: ["cs-101"],
      limit: 5,
    });
    expect(result.results).toEqual([
      {
        id: "cs-101",
        title: "CS 101 lecture",
        excerpt: "midterm review",
        score: 0.8,
        created_at: Date.parse("2026-08-01T00:00:00.000Z"),
      },
    ]);
  });

  it("filters folder meetings by event started_at, not session created_at", async () => {
    const search = vi.fn();
    const meetingSearchTool = buildSearchMeetingsTool({
      search,
      getFolderFilter: () => "CS 101",
    } as any);
    const startedAt = "2026-08-15T10:00:00.000Z";
    const createdAt = "2025-01-01T00:00:00.000Z";

    vi.mocked(loadSessionSummariesByFolder).mockResolvedValue([
      {
        id: "cs-101",
        title: "CS 101 lecture",
        created_at: createdAt,
        event_json: JSON.stringify({ started_at: startedAt }),
      },
    ]);

    const result = await (meetingSearchTool as any).execute({
      query: "",
      filters: {
        created_at: {
          kind: "absolute",
          gte: Date.parse("2026-08-14T00:00:00.000Z"),
          lte: Date.parse("2026-08-16T00:00:00.000Z"),
        },
      },
    });

    expect(search).not.toHaveBeenCalled();
    expect(result.results).toEqual([
      {
        id: "cs-101",
        title: "CS 101 lecture",
        excerpt: "",
        score: 0,
        created_at: Date.parse(startedAt),
      },
    ]);
  });

  it("drops folder meetings whose event start is outside the date filter", async () => {
    const meetingSearchTool = buildSearchMeetingsTool({
      search: vi.fn(),
      getFolderFilter: () => "CS 101",
    } as any);

    vi.mocked(loadSessionSummariesByFolder).mockResolvedValue([
      {
        id: "cs-101",
        title: "Old lecture",
        created_at: "2026-08-15T10:00:00.000Z",
        event_json: JSON.stringify({
          started_at: "2025-01-01T00:00:00.000Z",
        }),
      },
    ]);

    const result = await (meetingSearchTool as any).execute({
      query: "",
      filters: {
        created_at: {
          kind: "absolute",
          gte: Date.parse("2026-08-14T00:00:00.000Z"),
          lte: Date.parse("2026-08-16T00:00:00.000Z"),
        },
      },
    });

    expect(result).toEqual({ results: [] });
  });

  it("returns no meetings when the active folder is empty", async () => {
    const search = vi.fn();
    const meetingSearchTool = buildSearchMeetingsTool({
      search,
      getFolderFilter: () => "CS 101",
    } as any);

    vi.mocked(loadSessionSummariesByFolder).mockResolvedValue([]);

    const result = await (meetingSearchTool as any).execute({
      query: "midterm",
    });

    expect(result).toEqual({ results: [] });
    expect(search).not.toHaveBeenCalled();
    expect(searchMeetingContent).not.toHaveBeenCalled();
  });
});
