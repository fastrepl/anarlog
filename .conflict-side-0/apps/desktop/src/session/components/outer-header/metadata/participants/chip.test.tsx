import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ParticipantChip } from "./chip";

import type { SessionParticipantRecord } from "~/session/queries";

const mocks = vi.hoisted(() => ({
  participant: null as SessionParticipantRecord | null,
  removeSessionParticipant: vi.fn(),
  removeHumanSpeakerAssignments: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: ReactNode }) => children,
  useLingui: () => ({ t: (value: string) => value }),
}));

vi.mock("~/session/queries", () => ({
  removeSessionParticipant: mocks.removeSessionParticipant,
  useSessionParticipant: () => mocks.participant,
}));

vi.mock("~/store/zustand/tabs/index", () => ({
  useTabs: {
    getState: () => ({ openNew: vi.fn() }),
  },
}));

vi.mock("~/stt/queries", () => ({
  removeHumanSpeakerAssignments: mocks.removeHumanSpeakerAssignments,
}));

function participant(
  overrides: Partial<SessionParticipantRecord> = {},
): SessionParticipantRecord {
  return {
    id: "mapping-1",
    sessionId: "session-1",
    humanId: "human-1",
    source: "manual",
    name: "Lex Fridman",
    email: "",
    jobTitle: "",
    linkedinUsername: "",
    organizationId: "",
    organizationName: "",
    ...overrides,
  };
}

describe("ParticipantChip", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.participant = null;
  });

  afterEach(() => {
    cleanup();
  });

  it("hides participants without a name or email", () => {
    mocks.participant = participant({ name: "", email: "" });

    const { container } = render(<ParticipantChip mappingId="mapping-1" />);

    expect(container.textContent).toBe("");
    expect(screen.queryByText("Unknown")).toBeNull();
  });

  it("uses the email when a participant has no name", () => {
    mocks.participant = participant({ name: "", email: "lex@example.com" });

    render(<ParticipantChip mappingId="mapping-1" />);

    expect(screen.getByText("lex@example.com")).toBeTruthy();
  });
});
