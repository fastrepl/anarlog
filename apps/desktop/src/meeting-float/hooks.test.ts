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
  loadMeetingFloatData,
  subscribeMeetingFloatData,
} from "./hooks";

const rows = [
  {
    row_kind: "participant",
    session_id: "session-1",
    title: "",
    owner_user_id: "human-self",
    human_id: "human-remote",
    human_name: "Remote speaker",
  },
  {
    row_kind: "human",
    session_id: "",
    title: "",
    owner_user_id: "",
    human_id: "human-other",
    human_name: "Other person",
  },
  {
    row_kind: "session",
    session_id: "session-1",
    title: "Planning",
    owner_user_id: "human-self",
    human_id: "",
    human_name: "",
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
});
