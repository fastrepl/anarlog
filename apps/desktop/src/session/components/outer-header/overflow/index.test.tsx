import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { OverflowButton } from "./index";

import { openFloatingMeetingPanel } from "~/meeting-float/host";
import type { EditorView } from "~/store/zustand/tabs/schema";

const {
  uploadAudioMock,
  uploadTranscriptMock,
  regenerateTranscriptMock,
  audioExists,
  audioExistsResolved,
  currentNoteContent,
  exportModalMock,
  useHasTranscriptMock,
  useListenerMock,
  useConfigValueMock,
  windowShowMock,
} = vi.hoisted(() => ({
  uploadAudioMock: vi.fn(),
  uploadTranscriptMock: vi.fn(),
  regenerateTranscriptMock: vi.fn(),
  audioExists: { value: false },
  audioExistsResolved: { value: true },
  currentNoteContent: { value: "" },
  exportModalMock: vi.fn(
    (_props: { open: boolean; onOpenChange: (open: boolean) => void }) => null,
  ),
  useHasTranscriptMock: vi.fn(),
  useListenerMock: vi.fn(),
  useConfigValueMock: vi.fn(),
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
  ExportModal: exportModalMock,
}));

vi.mock("./listening", () => ({
  Listening: ({ resume }: { resume: boolean }) => (
    <button type="button">
      {resume ? "Resume listening" : "Start listening"}
    </button>
  ),
}));

vi.mock("./misc", () => ({
  ShowInFinder: () => <button type="button">Show in Finder</button>,
}));

vi.mock("~/meeting-float/host", () => ({
  openFloatingMeetingPanel: vi.fn(),
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({
    audioExists: audioExists.value,
    audioExistsResolved: audioExistsResolved.value,
  }),
}));

vi.mock("~/session/components/note-input/transcript/actions", () => ({
  useRegenerateTranscript: () => regenerateTranscriptMock,
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowShow: windowShowMock,
  },
}));

vi.mock("~/session/components/shared", () => ({
  useCurrentNoteHasContent: () => currentNoteContent.value.trim().length > 0,
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
    audioExists.value = false;
    audioExistsResolved.value = true;
    currentNoteContent.value = "";
    useHasTranscriptMock.mockReturnValue(true);
    useConfigValueMock.mockReturnValue(false);
    useListenerMock.mockImplementation((selector) =>
      selector({
        getSessionMode: () => "inactive",
        stop: vi.fn(),
      }),
    );
  });

  it("keeps upload actions available when the current note is empty", () => {
    useHasTranscriptMock.mockReturnValue(false);

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

  it("offers audio upload for re-transcription when recording is missing", () => {
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
    fireEvent.click(
      screen.getByRole("button", { name: "Upload audio to re-transcribe" }),
    );
    expect(
      screen.getByRole("button", { name: "Resume listening" }),
    ).not.toBeNull();
    expect(uploadAudioMock).toHaveBeenCalledWith({
      preserveSessionDate: true,
    });
  });

  it("hides replacement upload until the audio lookup succeeds", () => {
    audioExistsResolved.value = false;

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Upload audio to re-transcribe" }),
    ).toBeNull();
  });

  it("hides initial upload actions until the audio lookup succeeds", () => {
    audioExistsResolved.value = false;
    useHasTranscriptMock.mockReturnValue(false);

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

  it.each(["active", "finalizing", "running_batch"])(
    "hides re-transcription actions while the session is %s",
    (sessionMode) => {
      useListenerMock.mockImplementation((selector) =>
        selector({
          getSessionMode: () => sessionMode,
        }),
      );

      render(
        <OverflowButton
          sessionId="session-1"
          currentView={{ type: "enhanced", id: "note-1" } as EditorView}
        />,
      );

      expect(
        screen.queryByRole("button", {
          name: "Upload audio to re-transcribe",
        }),
      ).toBeNull();
    },
  );

  it("renders one separator when meeting actions are disabled", () => {
    useHasTranscriptMock.mockReturnValue(false);
    currentNoteContent.value = "Existing content";

    const { container } = render(
      <OverflowButton
        allowListening={false}
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(container.querySelectorAll("hr")).toHaveLength(1);
  });

  it("offers resume and re-transcribe when recorded audio exists", () => {
    audioExists.value = true;
    useHasTranscriptMock.mockReturnValue(false);

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));

    expect(
      screen.getByRole("button", { name: "Resume listening" }),
    ).not.toBeNull();
    expect(regenerateTranscriptMock).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Upload transcript" }),
    ).toBeNull();
  });

  it("separates visible meeting actions from the static actions", () => {
    useHasTranscriptMock.mockReturnValue(false);

    const { container } = render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(container.querySelectorAll("hr")).toHaveLength(2);
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

  it("mounts the export modal only after export is selected", () => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 0;
    });

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    expect(exportModalMock).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    expect(exportModalMock).toHaveBeenCalledTimes(1);
  });

  it("keeps the export modal mounted after it closes", async () => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 0;
    });

    render(
      <OverflowButton
        sessionId="session-1"
        currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    exportModalMock.mock.lastCall?.[0].onOpenChange(false);

    await waitFor(() => {
      expect(exportModalMock).toHaveBeenLastCalledWith(
        expect.objectContaining({ open: false }),
        undefined,
      );
    });
  });

  it("hides upload actions when the current note has content", () => {
    useHasTranscriptMock.mockReturnValue(false);
    currentNoteContent.value = "Existing content";

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
    useHasTranscriptMock.mockReturnValue(false);
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
