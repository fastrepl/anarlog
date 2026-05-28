import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OverflowButton } from "./index";

import type { EditorView } from "~/store/zustand/tabs/schema";

const {
  uploadAudioMock,
  uploadTranscriptMock,
  useHasTranscriptMock,
  useListenerMock,
  useConfigValueMock,
} = vi.hoisted(() => ({
  uploadAudioMock: vi.fn(),
  uploadTranscriptMock: vi.fn(),
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
  Copy: () => <button type="button">Copy link</button>,
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
  beforeEach(() => {
    vi.clearAllMocks();
    useHasTranscriptMock.mockReturnValue(true);
    useConfigValueMock.mockReturnValue(false);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "inactive",
      }),
    );
  });

  it("keeps upload actions available when a transcript already exists", () => {
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
});
