import { describe, expect, it } from "vitest";

import {
  clearFailedChatGroupCreate,
  consumeFailedChatGroupCreate,
  isFailedChatGroupCreate,
  markFailedChatGroupCreate,
} from "./pending-persists";

describe("failed chat group persistence lifecycle", () => {
  it("keeps a terminal failure until its late stream completion consumes it", () => {
    markFailedChatGroupCreate("late-group");
    for (let index = 0; index < 512; index += 1) {
      markFailedChatGroupCreate(`newer-group-${index}`);
    }

    expect(isFailedChatGroupCreate("late-group")).toBe(true);
    expect(consumeFailedChatGroupCreate("late-group")).toBe(true);
    expect(isFailedChatGroupCreate("late-group")).toBe(false);
    expect(consumeFailedChatGroupCreate("late-group")).toBe(false);

    for (let index = 0; index < 512; index += 1) {
      clearFailedChatGroupCreate(`newer-group-${index}`);
    }
  });
});
