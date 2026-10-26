import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
  createSession: vi.fn(
    (_title: string, _userId?: unknown, _initial?: { raw_md: string }) =>
      Promise.resolve("session-new"),
  ),
  openNew: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

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
  useTabs: Object.assign(
    (selector: (state: { openCurrent: () => void }) => unknown) =>
      selector({
        openCurrent: vi.fn(),
      }),
    { getState: () => ({ openNew: mocks.openNew }) },
  ),
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
}));

import { TabContentEmpty } from "./empty";

describe("TabContentEmpty", () => {
  afterEach(() => {
    cleanup();
    mocks.platform.mockReturnValue("macos");
    mocks.createSession.mockClear();
    mocks.createSession.mockResolvedValue("session-new");
    mocks.openNew.mockClear();
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

  it("imports a dropped markdown file as a new note", async () => {
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

    const file = new File(["# Hello"], "notes.md", { type: "text/markdown" });
    file.text = () => Promise.resolve("# Hello");
    fireEvent.drop(screen.getByRole("button", { name: /New Note/ }), {
      dataTransfer: { files: [file], items: [], types: ["Files"] },
    });

    await waitFor(() => expect(mocks.createSession).toHaveBeenCalledTimes(1));
    const [title, userId, initial] = mocks.createSession.mock.calls[0]!;
    expect(title).toBe("notes");
    expect(userId).toBeUndefined();
    expect(JSON.parse(initial!.raw_md).type).toBe("doc");
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-new",
    });
  });

  it("swallows dropped non-markdown files without importing", () => {
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

    const file = new File(["x"], "notes.txt", { type: "text/plain" });
    const event = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "dataTransfer", {
      value: { files: [file], items: [], types: ["Files"] },
    });
    screen.getByRole("button", { name: /New Note/ }).dispatchEvent(event);

    expect(mocks.createSession).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });
});
