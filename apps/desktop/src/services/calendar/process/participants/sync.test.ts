import { afterEach, describe, expect, test, vi } from "vitest";

import type { ParticipantSyncSnapshot } from "../../storage";
import { syncSessionParticipants } from "./sync";

import { id } from "~/shared/utils";

vi.mock("~/shared/utils", () => ({
  id: vi.fn(() => "human-new"),
}));

function createSnapshot(
  overrides: Partial<ParticipantSyncSnapshot> = {},
): ParticipantSyncSnapshot {
  return {
    sessions: [],
    humans: [],
    mappings: [],
    ...overrides,
  };
}

const session = {
  id: "session-1",
  ownerUserId: "user-1",
  eventJson: JSON.stringify({ tracking_id: "tracking-1" }),
  trackingId: "tracking-1",
};

describe("syncSessionParticipants", () => {
  afterEach(() => {
    vi.mocked(id).mockReset().mockReturnValue("human-new");
  });

  test("updates participants on every note attached to a reconciled occurrence", () => {
    vi.mocked(id)
      .mockReturnValueOnce("human-one")
      .mockReturnValueOnce("human-two");
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "guest@example.com" }]],
      ]),
      snapshot: createSnapshot({
        sessions: [session, { ...session, id: "session-2" }],
      }),
    });
    expect(result.toAdd.map((mapping) => mapping.sessionId)).toEqual([
      "session-1",
      "session-2",
    ]);
    expect(result.humansToCreate).toHaveLength(1);
    expect(result.toAdd.map((mapping) => mapping.humanId)).toEqual([
      "human-one",
      "human-one",
    ]);
  });
  test("returns empty output when no events are provided", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map(),
      snapshot: createSnapshot(),
    });

    expect(result.toAdd).toEqual([]);
    expect(result.toDelete).toEqual([]);
    expect(result.humansToCreate).toEqual([]);
  });

  test("skips events without an associated session", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "test@example.com", name: "Test" }]],
      ]),
      snapshot: createSnapshot(),
    });

    expect(result.toAdd).toEqual([]);
    expect(result.humansToCreate).toEqual([]);
  });

  test("creates a human when the participant email is new", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "new@example.com", name: "New Person" }]],
      ]),
      snapshot: createSnapshot({ sessions: [session] }),
    });

    expect(result.humansToCreate).toEqual([
      {
        id: "human-new",
        ownerUserId: "user-1",
        email: "new@example.com",
        name: "New Person",
        companyName: "Example",
      },
    ]);
    expect(result.humansToEnrich).toEqual([]);
    expect(result.toAdd).toEqual([
      {
        sessionId: "session-1",
        humanId: "human-new",
        email: "new@example.com",
      },
    ]);
  });

  test("uses an existing human when email matches case-insensitively", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "Existing@Example.com", name: "Existing" }]],
      ]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [
          {
            id: "human-1",
            email: "existing@example.com",
            name: "Existing",
            organizationId: "org-1",
          },
        ],
      }),
    });

    expect(result.humansToCreate).toEqual([]);
    expect(result.toAdd[0]).toMatchObject({ humanId: "human-1" });
  });

  test("deletes auto mappings when a participant is removed", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([["tracking-1", []]]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [
          {
            id: "human-1",
            email: "removed@example.com",
            name: "",
            organizationId: "",
          },
        ],
        mappings: [
          {
            id: "mapping-1",
            sessionId: "session-1",
            humanId: "human-1",
            source: "auto",
          },
        ],
      }),
    });

    expect(result.toDelete).toEqual(["mapping-1"]);
  });

  test("preserves excluded mappings", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([["tracking-1", []]]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [
          {
            id: "human-1",
            email: "excluded@example.com",
            name: "",
            organizationId: "",
          },
        ],
        mappings: [
          {
            id: "mapping-1",
            sessionId: "session-1",
            humanId: "human-1",
            source: "excluded",
          },
        ],
      }),
    });

    expect(result.toDelete).toEqual([]);
  });

  test("derives a display name and company for new humans without a name", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "simon.goldstein@ionprotocol.io" }]],
      ]),
      snapshot: createSnapshot({ sessions: [session] }),
    });

    expect(result.humansToCreate).toEqual([
      {
        id: "human-new",
        ownerUserId: "user-1",
        email: "simon.goldstein@ionprotocol.io",
        name: "Simon Goldstein",
        companyName: "Ionprotocol",
      },
    ]);
  });

  test("does not attach a company for personal email domains", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "jane.doe@gmail.com" }]],
      ]),
      snapshot: createSnapshot({ sessions: [session] }),
    });

    expect(result.humansToCreate[0]).toEqual({
      id: "human-new",
      ownerUserId: "user-1",
      email: "jane.doe@gmail.com",
      name: "Jane Doe",
    });
  });

  test("enriches existing humans whose name is missing or an email", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        [
          "tracking-1",
          [
            { email: "alice@acme.com" },
            { email: "bob@acme.com", name: "Bob Builder" },
            { email: "done@acme.com" },
          ],
        ],
      ]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [
          {
            id: "human-a",
            email: "alice@acme.com",
            name: "alice@acme.com",
            organizationId: "",
          },
          {
            id: "human-b",
            email: "bob@acme.com",
            name: "",
            organizationId: "org-1",
          },
          {
            id: "human-c",
            email: "done@acme.com",
            name: "Done Person",
            organizationId: "org-1",
          },
        ],
      }),
    });

    expect(result.humansToCreate).toEqual([]);
    expect(result.humansToEnrich).toEqual([
      {
        id: "human-a",
        ownerUserId: "user-1",
        name: "Alice",
        companyName: "Acme",
      },
      { id: "human-b", ownerUserId: "user-1", name: "Bob Builder" },
    ]);
  });

  test("never enriches the session owner", () => {
    const result = syncSessionParticipants({
      incomingParticipants: new Map([
        ["tracking-1", [{ email: "me@acme.com" }]],
      ]),
      snapshot: createSnapshot({
        sessions: [session],
        humans: [
          { id: "user-1", email: "me@acme.com", name: "", organizationId: "" },
        ],
      }),
    });

    expect(result.humansToEnrich).toEqual([]);
  });
});
