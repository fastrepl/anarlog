import { describe, expect, it } from "vitest";

import { shouldClearTimelineSelectionOnPointerDown } from "./interaction";

describe("shouldClearTimelineSelectionOnPointerDown", () => {
  it("clears when the click is outside the timeline", () => {
    const button = document.createElement("button");
    button.textContent = "Transcript";
    document.body.append(button);

    expect(shouldClearTimelineSelectionOnPointerDown(button)).toBe(true);

    button.remove();
  });

  it("keeps selection for clicks inside the timeline", () => {
    const root = document.createElement("div");
    root.dataset.sidebarTimelineRoot = "";
    const item = document.createElement("button");
    root.append(item);
    document.body.append(root);

    expect(shouldClearTimelineSelectionOnPointerDown(item)).toBe(false);

    root.remove();
  });

  it("keeps selection for clicks inside a dialog", () => {
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    const cancel = document.createElement("button");
    dialog.append(cancel);
    document.body.append(dialog);

    expect(shouldClearTimelineSelectionOnPointerDown(cancel)).toBe(false);

    dialog.remove();
  });

  it("keeps selection for clicks on a dialog overlay", () => {
    const overlay = document.createElement("div");
    overlay.dataset.dialogOverlay = "";
    document.body.append(overlay);

    expect(shouldClearTimelineSelectionOnPointerDown(overlay)).toBe(false);

    overlay.remove();
  });
});
