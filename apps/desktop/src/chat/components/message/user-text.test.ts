import { describe, expect, test } from "vitest";

import { buildUserMessageSegments } from "./user-text";

describe("buildUserMessageSegments", () => {
  test("replaces matching mention tokens inline", () => {
    expect(
      buildUserMessageSegments("chat with @adhit@janet.ai about this", [
        {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "adhit@janet.ai",
          tokens: ["@adhit@janet.ai"],
        },
      ]),
    ).toEqual([
      { type: "text", text: "chat with " },
      {
        type: "mention",
        mention: {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "adhit@janet.ai",
          tokens: ["@adhit@janet.ai"],
        },
      },
      { type: "text", text: " about this" },
    ]);
  });

  test("renders repeated mentions from a single ref", () => {
    expect(
      buildUserMessageSegments("@Sam follow up with @Sam tomorrow", [
        {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "Sam",
          tokens: ["@Sam"],
        },
      ]),
    ).toEqual([
      {
        type: "mention",
        mention: {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "Sam",
          tokens: ["@Sam"],
        },
      },
      { type: "text", text: " follow up with " },
      {
        type: "mention",
        mention: {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "Sam",
          tokens: ["@Sam"],
        },
      },
      { type: "text", text: " tomorrow" },
    ]);
  });

  test("prefers the longest token when mentions overlap", () => {
    expect(
      buildUserMessageSegments("@Sam Lee and @Sam", [
        {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "Sam",
          tokens: ["@Sam"],
        },
        {
          key: "human:manual:2",
          kind: "human",
          entityId: "2",
          displayLabel: "Sam Lee",
          tokens: ["@Sam Lee", "@Sam"],
        },
      ]),
    ).toEqual([
      {
        type: "mention",
        mention: {
          key: "human:manual:2",
          kind: "human",
          entityId: "2",
          displayLabel: "Sam Lee",
          tokens: ["@Sam Lee", "@Sam"],
        },
      },
      { type: "text", text: " and " },
      {
        type: "mention",
        mention: {
          key: "human:manual:1",
          kind: "human",
          entityId: "1",
          displayLabel: "Sam",
          tokens: ["@Sam"],
        },
      },
    ]);
  });
});
