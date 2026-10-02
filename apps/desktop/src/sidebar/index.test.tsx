import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
  platform: "macos",
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("~/sidebar/folder-materials", () => ({
  FolderMaterialsPanel: ({ folderPath }: { folderPath: string }) => (
    <div data-testid="folder-materials" data-folder-path={folderPath} />
  ),
}));

vi.mock("~/sidebar/timeline", () => ({
  TimelineView: ({ folderFilter = null }: { folderFilter?: string | null }) => (
    <div data-testid="timeline-view" data-folder-filter={folderFilter ?? ""} />
  ),
}));

vi.mock("~/sidebar/calendar", () => ({
  CalendarNav: () => <div data-testid="calendar-nav" />,
}));

vi.mock("~/sidebar/automations", () => ({
  AutomationsNav: () => <div data-testid="automations-nav" />,
}));

vi.mock("~/sidebar/contacts", () => ({
  ContactsNav: () => <div data-testid="contacts-nav" />,
}));

vi.mock("~/sidebar/settings", () => ({
  SettingsNav: () => <div data-testid="settings-nav" />,
}));

vi.mock("~/sidebar/templates", () => ({
  TemplatesNav: () => <div data-testid="templates-nav" />,
}));

vi.mock("~/sidebar/folders", () => ({
  FoldersNav: () => <div data-testid="folders-nav" />,
}));

vi.mock("~/sidebar/shared-notes", () => ({
  SharedNotesNav: () => <div data-testid="shared-notes-nav" />,
}));

import { LeftSidebar } from "./index";

describe("LeftSidebar", () => {
  beforeEach(() => {
    mocks.currentTab = { type: "empty" };
    mocks.platform = "macos";
  });

  afterEach(() => {
    cleanup();
  });

  it("shows received notes without the personal timeline", () => {
    render(<LeftSidebar noteFilter="shared" />);

    expect(screen.queryByTestId("timeline-view")).toBeNull();
    expect(screen.getByTestId("shared-notes-nav")).toBeTruthy();
  });

  it("keeps the personal timeline when filtering to a folder", () => {
    render(<LeftSidebar folderFilter="CS 101" />);

    expect(screen.getByTestId("timeline-view")).toBeTruthy();
    expect(
      screen.getByTestId("timeline-view").getAttribute("data-folder-filter"),
    ).toBe("CS 101");
    expect(
      screen.getByTestId("folder-materials").getAttribute("data-folder-path"),
    ).toBe("CS 101");
    expect(screen.queryByTestId("shared-notes-nav")).toBeNull();
  });

  it("hides folder materials when viewing all notes", () => {
    render(<LeftSidebar />);

    expect(screen.queryByTestId("folder-materials")).toBeNull();
  });

  it.each([
    ["settings", "settings-nav"],
    ["calendar", "calendar-nav"],
    ["contacts", "contacts-nav"],
    ["templates", "templates-nav"],
    ["automations", "automations-nav"],
    ["folders", "folders-nav"],
  ])("shows the %s nav instead of the timeline", (type, testId) => {
    mocks.currentTab = { type };

    render(<LeftSidebar />);

    expect(screen.getByTestId(testId)).toBeTruthy();
    expect(screen.queryByTestId("timeline-view")).toBeNull();
  });
});
