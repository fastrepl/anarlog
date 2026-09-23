import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  subscribe: vi.fn(),
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: mocks.execute,
    subscribe: mocks.subscribe,
  },
}));

import {
  createMeetingFloatLabelContext,
  createMeetingFloatRenderRequest,
  loadMeetingFloatData,
  subscribeMeetingFloatData,
} from "./hooks";

const speakerContextJson = JSON.stringify({
  intervals: [
    {
      start_ms: 1000,
      end_ms: 60000,
      active_call: true,
      calendar_call: false,
      mic_isolated: null,
      shared_microphone: false,
      title: "",
      self_names: [],
      participants: [{ human_id: "human-remote", name: "Remote speaker" }],
    },
  ],
});

const rows = [
  {
    row_kind: "participant",
    session_id: "session-1",
    title: "",
    owner_user_id: "human-self",
    human_id: "human-remote",
    human_name: "Remote speaker",
    speaker_context: "",
    started_at: "",
  },
  {
    row_kind: "human",
    session_id: "",
    title: "",
    owner_user_id: "",
    human_id: "human-other",
    human_name: "Other person",
    speaker_context: "",
    started_at: "",
  },
  {
    row_kind: "session",
    session_id: "session-1",
    title: "Planning",
    owner_user_id: "human-self",
    human_id: "",
    human_name: "",
    speaker_context: speakerContextJson,
    started_at: "2026-01-01T00:00:00.000Z",
  },
] as const;

describe("meeting float SQLite data", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads titles and speaker identity from one canonical snapshot", async () => {
    mocks.execute.mockResolvedValue(rows);

    const data = await loadMeetingFloatData();
    const labels = createMeetingFloatLabelContext(data, "session-1");

    expect(data.sessions["session-1"]).toEqual({
      title: "Planning",
      ownerUserId: "human-self",
      participantHumanIds: ["human-remote"],
      speakerContext: {
        intervals: [expect.objectContaining({ start_ms: 1000, end_ms: 60000 })],
      },
      startedAtMs: Date.parse("2026-01-01T00:00:00.000Z"),
    });
    expect(labels.getSelfHumanId()).toBe("human-self");
    expect(labels.getParticipantHumanIds?.()).toEqual(["human-remote"]);
    expect(labels.getHumanName("human-remote")).toBe("Remote speaker");
    expect(labels.getHumanName("human-other")).toBe("Other person");
    expect(mocks.execute.mock.calls[0][0]).toContain("session_participants");
  });

  it("maps live query updates through the same snapshot shape", async () => {
    const unsubscribe = vi.fn().mockResolvedValue(undefined);
    mocks.subscribe.mockImplementation(async (_sql, _params, handlers) => {
      handlers.onData(rows);
      return unsubscribe;
    });
    const onData = vi.fn();

    await expect(subscribeMeetingFloatData(onData, vi.fn())).resolves.toBe(
      unsubscribe,
    );
    expect(onData).toHaveBeenCalledWith(
      expect.objectContaining({
        sessions: expect.objectContaining({
          "session-1": expect.objectContaining({ title: "Planning" }),
        }),
      }),
    );
  });

  it("builds a render request from the session speaker context", async () => {
    mocks.execute.mockResolvedValue(rows);
    const data = await loadMeetingFloatData();
    const startedAt = data.sessions["session-1"]!.startedAtMs;

    const request = createMeetingFloatRenderRequest(
      data,
      "session-1",
      startedAt,
    );

    expect(request).toMatchObject({
      transcripts: [{ started_at: startedAt, words: [], assignments: [] }],
      participant_human_ids: ["human-remote"],
      self_human_id: "human-self",
      humans: [
        { human_id: "human-self", name: "" },
        { human_id: "human-remote", name: "Remote speaker" },
      ],
      speaker_context: {
        intervals: [expect.objectContaining({ start_ms: 1000 })],
      },
    });
  });

  it("returns no render request without a speaker context", async () => {
    mocks.execute.mockResolvedValue(
      rows.map((row) =>
        row.row_kind === "session" ? { ...row, speaker_context: "" } : row,
      ),
    );
    const data = await loadMeetingFloatData();

    expect(createMeetingFloatRenderRequest(data, "session-1", 0)).toBeNull();
  });
});
