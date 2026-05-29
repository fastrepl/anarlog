import { render, screen } from "@testing-library/react";
import type React from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("~/shared/main", () => ({
  StandardTabWrapper: ({
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
});
