import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

import {
  getMuteState,
  getParticipantNames,
  isMeetPageActive,
  isSelfParticipant,
  normalizeParticipantName,
} from "../../entrypoints/meet.content/dom";

function loadFixture(name: string) {
  const filePath = path.resolve(import.meta.dirname, "fixtures", name);
  return fs.readFileSync(filePath, "utf8");
}

function renderFixture(name: string) {
  document.body.innerHTML = loadFixture(name);
}

describe("getMuteState", () => {
  it("returns false when mic is unmuted", () => {
    renderFixture("meet-active-unmuted.html");
    expect(getMuteState(document)).toBe(false);
  });

  it("returns true when data-is-muted is true", () => {
    renderFixture("meet-active-muted.html");
    expect(getMuteState(document)).toBe(true);
  });

  it("returns true when aria-pressed is true", () => {
    renderFixture("meet-self-and-filtered.html");
    expect(getMuteState(document)).toBe(true);
  });

  it("defaults to false when no mute control exists", () => {
    renderFixture("meet-lobby.html");
    expect(getMuteState(document)).toBe(false);
  });
});

describe("getParticipantNames", () => {
  it("extracts participants from real-looking DOM", () => {
    renderFixture("meet-active-unmuted.html");

    expect(getParticipantNames(document)).toEqual([
      { name: "Alice Kim", is_self: false },
      { name: "Bob Stone", is_self: false },
    ]);
  });

  it("detects self participants and filters invalid names", () => {
    renderFixture("meet-self-and-filtered.html");

    expect(getParticipantNames(document)).toEqual([
      { name: "Casey Rivera", is_self: true },
      { name: "Taylor (You)", is_self: true },
    ]);
  });

  it("caps participants at the configured maximum", () => {
    renderFixture("meet-many-participants.html");

    const participants = getParticipantNames(document);
    expect(participants).toHaveLength(30);
    expect(participants[0]).toEqual({ name: "Participant 01", is_self: false });
    expect(participants[29]).toEqual({
      name: "Participant 30",
      is_self: false,
    });
  });

  it("deduplicates repeated participant ids", () => {
    document.body.innerHTML = `
      <div data-participant-id="p1"><span>Alice Kim</span></div>
      <div data-participant-id="p1"><span>Alice Kim</span></div>
    `;

    expect(getParticipantNames(document)).toEqual([
      { name: "Alice Kim", is_self: false },
    ]);
  });
});

describe("isMeetPageActive", () => {
  it("returns true for a meet code pathname", () => {
    renderFixture("meet-lobby.html");
    expect(
      isMeetPageActive(
        { hostname: "meet.google.com", pathname: "/abc-defg-hij" },
        document,
      ),
    ).toBe(true);
  });

  it("returns true when call controls exist", () => {
    renderFixture("meet-active-unmuted.html");
    expect(
      isMeetPageActive(
        { hostname: "meet.google.com", pathname: "/landing" },
        document,
      ),
    ).toBe(true);
  });

  it("returns false for non-meet domains", () => {
    renderFixture("meet-active-unmuted.html");
    expect(
      isMeetPageActive(
        { hostname: "example.com", pathname: "/abc-defg-hij" },
        document,
      ),
    ).toBe(false);
  });

  it("returns false for meet lobby without controls", () => {
    renderFixture("meet-lobby.html");
    expect(
      isMeetPageActive(
        { hostname: "meet.google.com", pathname: "/landing" },
        document,
      ),
    ).toBe(false);
  });
});

describe("normalizeParticipantName", () => {
  it("normalizes whitespace", () => {
    expect(normalizeParticipantName("  Alice    Kim  ")).toBe("Alice Kim");
  });

  it("rejects control-like values", () => {
    expect(normalizeParticipantName("Turn on microphone")).toBeNull();
  });
});

describe("isSelfParticipant", () => {
  it("detects you markers", () => {
    expect(isSelfParticipant("You")).toBe(true);
    expect(isSelfParticipant("Alex (You)")).toBe(true);
  });

  it("does not flag regular names", () => {
    expect(isSelfParticipant("Yousef")).toBe(false);
  });
});
