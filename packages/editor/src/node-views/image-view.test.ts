import { describe, expect, it, vi } from "vitest";

import { listenForImageResize } from "./image-view";

describe("listenForImageResize", () => {
  it("releases every listener after pointer cancellation", () => {
    const onMove = vi.fn();
    const onCommit = vi.fn();
    const onCancel = vi.fn();
    listenForImageResize({ onMove, onCommit, onCancel });

    window.dispatchEvent(new Event("pointermove"));
    window.dispatchEvent(new Event("pointercancel"));
    window.dispatchEvent(new Event("pointermove"));
    window.dispatchEvent(new Event("pointerup"));

    expect(onMove).toHaveBeenCalledTimes(1);
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("returned cleanup releases listeners without committing", () => {
    const onMove = vi.fn();
    const onCommit = vi.fn();
    const onCancel = vi.fn();
    const cleanup = listenForImageResize({ onMove, onCommit, onCancel });

    cleanup();
    window.dispatchEvent(new Event("pointermove"));
    window.dispatchEvent(new Event("pointerup"));

    expect(onMove).not.toHaveBeenCalled();
    expect(onCommit).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });
});
