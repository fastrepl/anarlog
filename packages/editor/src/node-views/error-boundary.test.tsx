import { cleanup, render, screen } from "@testing-library/react";
import { afterAll, afterEach, describe, expect, it, vi } from "vitest";

import { getSafeNodePos, withNodeViewErrorBoundary } from "./error-boundary";

const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
const renderOptions = {
  onCaughtError: () => {},
  onRecoverableError: () => {},
} satisfies Parameters<typeof render>[1];

afterEach(() => {
  cleanup();
  consoleError.mockClear();
});

afterAll(() => {
  consoleError.mockRestore();
});

describe("getSafeNodePos", () => {
  it("returns null when ProseMirror no longer has a node position", () => {
    expect(
      getSafeNodePos(() => {
        throw new Error("detached node");
      }),
    ).toBeNull();
  });
});

describe("withNodeViewErrorBoundary", () => {
  it("contains node view render failures to a local fallback", () => {
    function BrokenNodeView() {
      throw new Error("node view failed");
    }

    const Wrapped = withNodeViewErrorBoundary<HTMLSpanElement>(BrokenNodeView, {
      name: "attachment",
    });

    render(
      <Wrapped
        nodeProps={
          {
            node: {
              attrs: {},
              textContent: "attachment.txt",
            },
          } as any
        }
      />,
      renderOptions,
    );

    const fallback = screen.getByText("attachment.txt");
    expect(fallback.getAttribute("data-node-view-error")).toBe("attachment");
  });
});
