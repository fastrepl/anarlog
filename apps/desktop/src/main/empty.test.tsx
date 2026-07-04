import { cleanup, render, screen } from "@testing-library/react";
import type React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({
    children,
    floatingButton,
  }: {
    children: React.ReactNode;
    floatingButton?: React.ReactNode;
  }) => (
    <div>
      {children}
      {floatingButton}
    </div>
  ),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
  useNewNoteAndListen: () => vi.fn(),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: "FloatingClosed",
      sendEvent: vi.fn(),
    },
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openCurrent: () => void }) => unknown) =>
    selector({
      openCurrent: vi.fn(),
    }),
}));

import { TabContentEmpty } from "./empty";

describe("TabContentEmpty", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows the home actions and global chat FAB", () => {
    render(
      <TabContentEmpty
        tab={{
          active: true,
          pinned: false,
          slotId: "slot-home",
          type: "empty",
        }}
      />,
    );

    expect(screen.getByRole("button", { name: /New Note/ })).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeTruthy();
  });

  it("centers actions in a draggable empty surface while keeping actions clickable", () => {
    render(
      <TabContentEmpty
        tab={{
          active: true,
          pinned: false,
          slotId: "slot-home",
          type: "empty",
        }}
      />,
    );

    const newNoteButton = screen.getByRole("button", { name: /New Note/ });
    const dragSurface = newNoteButton.parentElement?.parentElement;

    expect(dragSurface?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(dragSurface?.className).not.toContain("mb-12");
    expect(newNoteButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(
      screen
        .getByRole("button", { name: /Start Recording/ })
        .getAttribute("data-tauri-drag-region"),
    ).toBe("false");
    expect(
      screen
        .getByRole("button", { name: /Settings/ })
        .getAttribute("data-tauri-drag-region"),
    ).toBe("false");
  });
});
