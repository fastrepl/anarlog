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
} = vi.hoisted(() => ({
  uploadAudioMock: vi.fn(),
  uploadTranscriptMock: vi.fn(),
  useCurrentNoteHasContentMock: vi.fn(),
  useHasTranscriptMock: vi.fn(),
  useListenerMock: vi.fn(),
  useConfigValueMock: vi.fn(),
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
  DeleteRecording: () => <button type="button">Delete recording</button>,
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

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({
    audioExists: true,
  }),
}));

vi.mock("~/meeting-float/host", () => ({
  openFloatingMeetingPanel: vi.fn(),
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

    expect(openFloatingMeetingPanel).toHaveBeenCalledWith({
      sessionId: "session-1",
      enabled: true,
    });
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
});
