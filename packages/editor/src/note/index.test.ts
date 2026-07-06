import { afterEach, describe, expect, it, vi } from "vitest";

import type { JSONContent } from "./index";
import {
  getEditorCompositionWaitMs,
  shouldReplaceEditorContent,
} from "./index";

const baseDoc: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "old" }] }],
};

const nextDoc: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "new" }] }],
};

afterEach(() => {
  vi.useRealTimers();
});

describe("shouldReplaceEditorContent", () => {
  it("does not replace content while IME composition is active", () => {
    expect(
      shouldReplaceEditorContent({
        currentContent: baseDoc,
        nextContent: nextDoc,
        hasFocus: true,
        isComposing: true,
        syncContentWhenFocused: true,
      }),
    ).toBe(false);
  });

  it("allows focused content sync after composition ends when enabled", () => {
    expect(
      shouldReplaceEditorContent({
        currentContent: baseDoc,
        nextContent: nextDoc,
        hasFocus: true,
        isComposing: false,
        syncContentWhenFocused: true,
      }),
    ).toBe(true);
  });
});

describe("getEditorCompositionWaitMs", () => {
  it("waits through active IME composition", () => {
    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: true, endedAt: 0 },
      ),
    ).toBe(500);
  });

  it("returns the remaining post-composition grace window", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);

    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: false, endedAt: 600 },
      ),
    ).toBe(100);
  });

  it("returns zero after the post-composition grace window", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);

    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: false, endedAt: 499 },
      ),
    ).toBe(0);
  });

  it("returns zero for a reset inactive composition state", () => {
    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: false, endedAt: 0 },
      ),
    ).toBe(0);
  });
});
