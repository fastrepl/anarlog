import { afterEach, describe, expect, it } from "vitest";

import { shouldClearTimelineSelectionOnPointerDown } from "./interaction";

function mount(build: () => { root: HTMLElement; target: HTMLElement }) {
  const { root, target } = build();
  document.body.append(root);
  return target;
}

describe("shouldClearTimelineSelectionOnPointerDown", () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  it.each([
    [
      "outside the timeline",
      true,
      () => {
        const button = document.createElement("button");
        return { root: button, target: button };
      },
    ],
    [
      "inside the timeline",
      false,
      () => {
        const root = document.createElement("div");
        root.dataset.sidebarTimelineRoot = "";
        const item = document.createElement("button");
        root.append(item);
        return { root, target: item };
      },
    ],
    [
      "inside a dialog",
      false,
      () => {
        const root = document.createElement("div");
        root.setAttribute("role", "dialog");
        const cancel = document.createElement("button");
        root.append(cancel);
        return { root, target: cancel };
      },
    ],
    [
      "on a dialog overlay",
      false,
      () => {
        const overlay = document.createElement("div");
        overlay.dataset.dialogOverlay = "";
        return { root: overlay, target: overlay };
      },
    ],
  ])("pointer down %s clears selection: %s", (_, shouldClear, build) => {
    expect(shouldClearTimelineSelectionOnPointerDown(mount(build))).toBe(
      shouldClear,
    );
  });
});
