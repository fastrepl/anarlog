import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chat: {
    mode: "FloatingClosed",
    sendEvent: vi.fn(),
  },
  close: vi.fn(),
  leftsidebar: {
    expanded: true,
  },
  sidebarTimelineEnabled: false,
}));

vi.mock("@hypr/changelog", () => ({
  ChangelogContent: ({ content }: { content: string }) => <div>{content}</div>,
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openUrl: vi.fn(),
  },
}));

vi.mock("./data", () => ({
  useChangelogContent: () => ({
    content: "Release notes",
    loading: false,
  }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: mocks.chat,
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.sidebarTimelineEnabled,
}));

vi.mock("~/shared/main", () => ({
  StandardTabWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { close: typeof mocks.close }) => unknown) =>
    selector({ close: mocks.close }),
}));

import { TabContentChangelog } from "./index";

import type { Tab } from "~/store/zustand/tabs";

describe("TabContentChangelog", () => {
  beforeEach(() => {
    mocks.chat.mode = "FloatingClosed";
    mocks.chat.sendEvent.mockClear();
    mocks.close.mockClear();
    mocks.leftsidebar.expanded = true;
    mocks.sidebarTimelineEnabled = false;
  });

  afterEach(() => {
    cleanup();
  });

  it("adds the note header gutter when sidebar timeline mode is collapsed", () => {
    mocks.sidebarTimelineEnabled = true;
    mocks.leftsidebar.expanded = false;

    render(<TabContentChangelog tab={buildChangelogTab()} />);

    const heading = screen.getByRole("heading", {
      name: "What's new in 1.0.36?",
    });
    const titleSlot = heading.parentElement;

    expect(getHeader().className).toContain("pl-[156px]");
    expect(titleSlot?.className).toContain("left-[104px]");
    expect(titleSlot?.className).not.toContain("-translate-y-1");
    expect(titleSlot?.className).toContain("right-[70px]");
    expect(titleSlot?.className).toContain("justify-start");
    expect(titleSlot?.className).not.toContain("left-1/2");
    expect(heading.className).toContain("text-left");
  });

  it("does not add the collapsed sidebar gutter while the left sidebar is expanded", () => {
    mocks.sidebarTimelineEnabled = true;

    render(<TabContentChangelog tab={buildChangelogTab()} />);

    expect(getHeader().className).not.toContain("pl-[156px]");
  });

  it("uses the left-edge title slot while sidebar timeline mode is expanded", () => {
    mocks.sidebarTimelineEnabled = true;
    mocks.leftsidebar.expanded = true;

    render(<TabContentChangelog tab={buildChangelogTab()} />);

    const heading = screen.getByRole("heading", {
      name: "What's new in 1.0.36?",
    });
    const titleSlot = heading.parentElement;

    expect(titleSlot?.className).toContain("left-0");
    expect(titleSlot?.className).toContain("right-[70px]");
    expect(titleSlot?.className).toContain("justify-start");
    expect(titleSlot?.className).not.toContain("left-1/2");
    expect(heading.className).toContain("text-left");
  });

  it("centers the changelog title independently from the close button", () => {
    render(<TabContentChangelog tab={buildChangelogTab()} />);

    const heading = screen.getByRole("heading", {
      name: "What's new in 1.0.36?",
    });
    const titleSlot = heading.parentElement;

    expect(titleSlot?.className).toContain("left-1/2");
    expect(titleSlot?.className).toContain("-translate-x-1/2");
    expect(heading.className).toContain("text-center");
  });
});

function getHeader() {
  const heading = screen.getByRole("heading", {
    name: "What's new in 1.0.36?",
  });

  return heading.parentElement?.parentElement as HTMLElement;
}

function buildChangelogTab(): Extract<Tab, { type: "changelog" }> {
  return {
    active: true,
    pinned: false,
    slotId: "slot-1",
    state: {
      current: "1.0.36",
      previous: null,
    },
    type: "changelog",
  };
}
