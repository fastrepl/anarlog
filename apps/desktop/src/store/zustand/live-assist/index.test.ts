import { beforeEach, describe, expect, it } from "vitest";

import { useLiveAssistStore } from "./index";

describe("useLiveAssistStore", () => {
  beforeEach(() => {
    useLiveAssistStore.setState({ cardsBySession: {} });
  });

  it("adds a generating card to the front of the session's list", () => {
    const { addGeneratingCard } = useLiveAssistStore.getState();
    addGeneratingCard("session-1", {
      id: "a",
      kind: "catch_up",
      createdAtMs: 1,
    });
    addGeneratingCard("session-1", {
      id: "b",
      kind: "action_items",
      createdAtMs: 2,
    });

    const cards = useLiveAssistStore.getState().cardsBySession["session-1"];
    expect(cards?.map((card) => card.id)).toEqual(["b", "a"]);
    expect(cards?.every((card) => card.status === "generating")).toBe(true);
  });

  it("resolves a card to ready or error in place", () => {
    const { addGeneratingCard, resolveCard } = useLiveAssistStore.getState();
    addGeneratingCard("session-1", {
      id: "a",
      kind: "catch_up",
      createdAtMs: 1,
    });

    resolveCard("session-1", "a", { status: "ready", items: ["Item one."] });

    expect(
      useLiveAssistStore.getState().cardsBySession["session-1"]?.[0],
    ).toMatchObject({ status: "ready", items: ["Item one."] });

    resolveCard("session-1", "a", {
      status: "error",
      errorMessage: "failed",
    });

    expect(
      useLiveAssistStore.getState().cardsBySession["session-1"]?.[0],
    ).toMatchObject({ status: "error", errorMessage: "failed" });
  });

  it("removes a single card without touching others", () => {
    const { addGeneratingCard, removeCard } = useLiveAssistStore.getState();
    addGeneratingCard("session-1", {
      id: "a",
      kind: "catch_up",
      createdAtMs: 1,
    });
    addGeneratingCard("session-1", {
      id: "b",
      kind: "follow_up",
      createdAtMs: 2,
    });

    removeCard("session-1", "a");

    expect(
      useLiveAssistStore
        .getState()
        .cardsBySession["session-1"]?.map((card) => card.id),
    ).toEqual(["b"]);
  });

  it("caps the number of cards retained per session", () => {
    const { addGeneratingCard } = useLiveAssistStore.getState();
    for (let index = 0; index < 25; index += 1) {
      addGeneratingCard("session-1", {
        id: `card-${index}`,
        kind: "catch_up",
        createdAtMs: index,
      });
    }

    expect(
      useLiveAssistStore.getState().cardsBySession["session-1"],
    ).toHaveLength(20);
    expect(
      useLiveAssistStore.getState().cardsBySession["session-1"]?.[0]?.id,
    ).toBe("card-24");
  });

  it("clears only the requested session", () => {
    const { addGeneratingCard, clearSession } = useLiveAssistStore.getState();
    addGeneratingCard("session-1", {
      id: "a",
      kind: "catch_up",
      createdAtMs: 1,
    });
    addGeneratingCard("session-2", {
      id: "b",
      kind: "catch_up",
      createdAtMs: 1,
    });

    clearSession("session-1");

    const { cardsBySession } = useLiveAssistStore.getState();
    expect(cardsBySession["session-1"]).toBeUndefined();
    expect(cardsBySession["session-2"]).toHaveLength(1);
  });
});
