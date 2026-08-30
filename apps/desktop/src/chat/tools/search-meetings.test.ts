import { describe, expect, it, vi } from "vitest";

import { buildSearchMeetingsTool } from "./search-meetings";

import { loadSessionSummariesByFolder } from "~/session/queries";

vi.mock("~/session/queries", () => ({
  loadSessionSummariesByFolder: vi.fn(),
}));

describe("search meetings chat tool", () => {
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

  it("keeps search results inside the active folder", async () => {
    const search = vi.fn().mockResolvedValue([
      {
        score: 0.9,
        document: {
          id: "other-folder",
          type: "session",
          title: "Other class",
          content: "midterm review",
          created_at: 100,
        },
      },
      {
        score: 0.8,
        document: {
          id: "cs-101",
          type: "session",
          title: "CS 101 lecture",
          content: "midterm review",
          created_at: 200,
        },
      },
    ]);
    const meetingSearchTool = buildSearchMeetingsTool({
      search,
      getFolderFilter: () => "CS 101",
    } as any);

    vi.mocked(loadSessionSummariesByFolder).mockResolvedValue([
      {
        id: "cs-101",
        title: "CS 101 lecture",
        created_at: "2026-08-01T00:00:00.000Z",
      },
    ]);

    const result = await (meetingSearchTool as any).execute({
      query: "midterm",
    });

    expect(result.results).toEqual([
      expect.objectContaining({ id: "cs-101", title: "CS 101 lecture" }),
    ]);
  });
});
