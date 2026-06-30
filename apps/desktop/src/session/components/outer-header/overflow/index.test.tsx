import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { OverflowButton } from "./index";

import { openFloatingMeetingPanel } from "~/meeting-float/host";
import type { EditorView } from "~/store/zustand/tabs/schema";

const {
  uploadAudioMock,
  uploadTranscriptMock,
  useCurrentNoteHasContentMock,
  useHasTranscriptMock,
  useListenerMock,
  useConfigValueMock,
  useMeetingFloatMainStoreMock,
  mainStoreMock,
  windowShowMock,
} = vi.hoisted(() => ({
  uploadAudioMock: vi.fn(),
  uploadTranscriptMock: vi.fn(),
  useCurrentNoteHasContentMock: vi.fn(),
  useHasTranscriptMock: vi.fn(),
  useListenerMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  useMeetingFloatMainStoreMock: vi.fn(),
  mainStoreMock: { getCell: vi.fn() },
  windowShowMock: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
}));

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@hypr/ui/components/ui/dropdown-menu", () => ({
  AppFloatingPanel: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenu: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuItem: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("./delete", () => ({
  DeleteNote: () => <button type="button">Delete note</button>,
}));

vi.mock("./export-modal", () => ({
  ExportModal: () => null,
}));

vi.mock("./listening", () => ({
  Listening: () => <button type="button">Resume listening</button>,
}));

vi.mock("./misc", () => ({
  ShowInFinder: () => <button type="button">Show in Finder</button>,
}));

vi.mock("~/meeting-float/host", () => ({
  openFloatingMeetingPanel: vi.fn(),
}));

vi.mock("~/meeting-float/hooks", () => ({
  useMeetingFloatMainStore: useMeetingFloatMainStoreMock,
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowShow: windowShowMock,
  },
}));

vi.mock("~/session/components/shared", () => ({
  useCurrentNoteHasContent: useCurrentNoteHasContentMock,
  useHasTranscript: useHasTranscriptMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("~/stt/useUploadFile", () => ({
  useUploadFile: vi.fn(() => ({
    uploadAudio: uploadAudioMock,
    uploadTranscript: uploadTranscriptMock,
  })),
}));

describe("OverflowButton", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    useCurrentNoteHasContentMock.mockReturnValue(false);
    useHasTranscriptMock.mockReturnValue(true);
    useConfigValueMock.mockReturnValue(false);
    useMeetingFloatMainStoreMock.mockReturnValue(mainStoreMock);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "inactive",
      }),
    );
  });

  it("keeps upload actions available when the current note is empty", () => {
    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Upload audio" }));
    fireEvent.click(screen.getByRole("button", { name: "Upload transcript" }));

    expect(uploadAudioMock).toHaveBeenCalledTimes(1);
    expect(uploadTranscriptMock).toHaveBeenCalledTimes(1);
  });

  it("keeps the overflow trigger out of the header drag region", () => {
    const { container } = render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    const trigger = container.querySelector(
      "button[data-tauri-drag-region='false']",
    );

    expect(trigger).not.toBeNull();
  });

  it("hides upload actions when the current note has content", () => {
    useCurrentNoteHasContentMock.mockReturnValue(true);

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Upload transcript" }),
    ).toBeNull();
  });

  it("hides upload actions while a meeting is in progress", () => {
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "active",
      }),
    );

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Upload transcript" }),
    ).toBeNull();
  });

  it("opens the floating panel while actively listening", () => {
    useConfigValueMock.mockReturnValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "active",
      }),
    );

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Open floating panel" }),
    );

    expect(openFloatingMeetingPanel).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: "session-1",
        enabled: true,
        main: mainStoreMock,
      }),
    );
  });

  it("opens the current note in a standalone window", () => {
    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open in New Window" }));

    expect(windowShowMock).toHaveBeenCalledWith({
      type: "note",
      value: "session-1",
    });
  });

  it("hides the standalone window action in standalone windows", () => {
    render(
      <OverflowButton
        standaloneWindow
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Open in New Window" }),
    ).toBeNull();
  });

  it("hides listening actions when listening is disabled", () => {
    useConfigValueMock.mockReturnValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "active",
      }),
    );

    render(
      <OverflowButton
        allowListening={false}
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Resume listening" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Open floating panel" }),
    ).toBeNull();
  });

  it("hides the floating panel action while finalizing", () => {
    useConfigValueMock.mockReturnValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "finalizing",
      }),
    );

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Open floating panel" }),
    ).toBeNull();
  });

  it("does not show the delete recording action", () => {
    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Delete recording" }),
    ).toBeNull();
    expect(screen.getByRole("button", { name: "Delete note" })).not.toBeNull();
  });
});
