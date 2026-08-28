import * as stylex from "@stylexjs/stylex";
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
}));

vi.mock("@anlg/changelog", () => ({
  ChangelogContent: ({ content }: { content: string }) => <div>{content}</div>,
}));

vi.mock("@anlg/plugin-opener2", () => ({
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

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { close: typeof mocks.close }) => unknown) =>
    selector({ close: mocks.close }),
}));

import { changelogStyles, TabContentChangelog } from "./index";

import type { Tab } from "~/store/zustand/tabs";

describe("TabContentChangelog", () => {
  beforeEach(() => {
    mocks.chat.mode = "FloatingClosed";
    mocks.chat.sendEvent.mockClear();
    mocks.close.mockClear();
    mocks.leftsidebar.expanded = true;
  });

  afterEach(() => {
    cleanup();
  });

  it("adds the note header gutter when the sidebar is collapsed", () => {
    mocks.leftsidebar.expanded = false;

    render(<TabContentChangelog tab={buildChangelogTab()} />);

    const heading = screen.getByRole("heading", {
      name: "What's new in 1.0.36?",
    });
    const titleSlot = heading.parentElement;

    expectStyle(getHeader(), changelogStyles.headerMacosGutter);
    expectStyle(titleSlot, changelogStyles.titleSlotGutter);
    expectStyle(titleSlot, changelogStyles.titleSlotMacos);
    expectStyle(heading, changelogStyles.titleLeft);
  });

  it("does not add the collapsed sidebar gutter while the left sidebar is expanded", () => {
    render(<TabContentChangelog tab={buildChangelogTab()} />);

    expectNotStyle(getHeader(), changelogStyles.headerMacosGutter);
  });

  it("uses the left-edge title slot while the sidebar is expanded", () => {
    mocks.leftsidebar.expanded = true;

    render(<TabContentChangelog tab={buildChangelogTab()} />);

    const heading = screen.getByRole("heading", {
      name: "What's new in 1.0.36?",
    });
    const titleSlot = heading.parentElement;

    expectStyle(titleSlot, changelogStyles.titleSlotExpanded);
    expectStyle(heading, changelogStyles.titleLeft);
  });

  it("marks the full top header area as draggable while keeping close clickable", () => {
    render(<TabContentChangelog tab={buildChangelogTab()} />);

    const header = getHeader();
    const headerFrame = header.parentElement;
    const heading = screen.getByRole("heading", {
      name: "What's new in 1.0.36?",
    });
    const titleSlot = heading.parentElement;
    const closeButton = screen.getByRole("button", {
      name: "Close changelog",
    });

    expect(headerFrame?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(header.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(titleSlot?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(closeButton.getAttribute("data-tauri-drag-region")).toBe("false");
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

function expectStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(true);
  }
}

function expectNotStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(false);
  }
}
