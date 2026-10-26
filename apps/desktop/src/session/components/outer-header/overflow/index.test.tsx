import {
  act,
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
  exportModalMounts,
  exportModalMock,
  useHasTranscriptMock,
  useListenerMock,
  useConfigValueMock,
  platformMock,
  windowShowMock,
  audioExistMock,
  sessionDirMock,
  openPathMock,
  toastErrorMock,
  sessionRecord,
  authenticateMock,
} = vi.hoisted(() => ({
  uploadAudioMock: vi.fn(),
  uploadTranscriptMock: vi.fn(),
  regenerateTranscriptMock: vi.fn(),
  audioExists: { value: false },
  audioExistsResolved: { value: true },
  currentNoteContent: { value: "" },
  exportModalMounts: { value: 0 },
  exportModalMock: vi.fn(
    (_props: { open: boolean; onOpenChange: (open: boolean) => void }) => null,
  ),
  useHasTranscriptMock: vi.fn(),
  useListenerMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  platformMock: vi.fn(() => "macos"),
  windowShowMock: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  audioExistMock: vi.fn(),
  sessionDirMock: vi.fn(),
  openPathMock: vi.fn(),
  toastErrorMock: vi.fn(),
  sessionRecord: { value: null as { locked?: boolean } | null },
  authenticateMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: platformMock,
}));

vi.mock("@anlg/ui/components/ui/button", () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  appFloatingMenuPanelClassName: "overflow-hidden p-1.5",
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
  DropdownMenuPortal: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuSub: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuSubContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuSubTrigger: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>
      {children}
      <span aria-hidden>›</span>
    </button>
  ),
  DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("./always-on-top", () => ({
  AlwaysOnTop: () => <button type="button">Always on Top</button>,
}));

vi.mock("./delete", () => ({
  DeleteNote: () => <button type="button">Delete note</button>,
}));

vi.mock("./export-modal", async () => {
  const React = await import("react");
  return {
    ExportModal: (props: {
      open: boolean;
      onOpenChange: (open: boolean) => void;
    }) => {
      React.useEffect(() => {
        exportModalMounts.value += 1;
      }, []);
      exportModalMock(props);
      return null;
    },
  };
});

vi.mock("./listening", () => ({
  Listening: ({ resume }: { resume: boolean }) => (
    <button type="button">
      {resume ? "Resume listening" : "Start listening"}
    </button>
  ),
}));

vi.mock("./lock-note", () => ({
  LockNote: () => <button type="button">Lock Note</button>,
}));

vi.mock("../metadata", () => ({
  MetadataPanelContent: ({ sessionId }: { sessionId: string }) => (
    <div data-testid="meeting-info-popover">{sessionId}</div>
  ),
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

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    windowShow: windowShowMock,
  },
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    audioExist: audioExistMock,
    sessionDir: sessionDirMock,
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: {
    openPath: openPathMock,
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: {
    error: toastErrorMock,
  },
}));

vi.mock("~/session/queries", () => ({
  useSession: () => sessionRecord.value,
}));

vi.mock("~/lock/store", () => ({
  useAppLock: {
    getState: () => ({ authenticate: authenticateMock }),
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

type Scenario = {
  transcript?: boolean;
  content?: string;
  audio?: boolean;
  audioResolved?: boolean;
  mode?: string;
  floatingPanel?: boolean;
  platform?: string;
};

const ACTIONS = [
  "Upload audio",
  "Upload transcript",
  "Start listening",
  "Resume listening",
  "Re-transcribe",
  "Show Audio in Finder",
  "Show Audio in Explorer",
  "Show Audio in Folder",
  "Open floating panel",
  "Open in New Window",
  "Delete recording",
  "Delete note",
] as const;

function arrange({
  transcript = true,
  content = "",
  audio = false,
  audioResolved = true,
  mode = "inactive",
  floatingPanel = false,
  platform = "macos",
}: Scenario) {
  useHasTranscriptMock.mockReturnValue(transcript);
  currentNoteContent.value = content;
  audioExists.value = audio;
  audioExistsResolved.value = audioResolved;
  useConfigValueMock.mockReturnValue(floatingPanel);
  platformMock.mockReturnValue(platform);
  useListenerMock.mockImplementation((selector) =>
    selector({ getSessionMode: () => mode, stop: vi.fn() }),
  );
  sessionRecord.value = { locked: false };
  audioExistMock.mockResolvedValue({ status: "ok", data: true });
  sessionDirMock.mockResolvedValue({
    status: "ok",
    data: "/sessions/session-1",
  });
  openPathMock.mockResolvedValue({ status: "ok", data: null });
  authenticateMock.mockResolvedValue(true);
}

function renderOverflow(
  props: Partial<React.ComponentProps<typeof OverflowButton>> = {},
) {
  return render(
    <OverflowButton
      sessionId="session-1"
      currentView={{ type: "enhanced", id: "note-1" } as EditorView}
      {...props}
    />,
  );
}

function visibleActions() {
  return ACTIONS.filter((name) => screen.queryByRole("button", { name }));
}

describe("OverflowButton", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    exportModalMounts.value = 0;
    arrange({});
  });

  it.each<
    [
      string,
      Scenario,
      Partial<React.ComponentProps<typeof OverflowButton>>,
      string[],
    ]
  >([
    [
      "empty note without a recording",
      { transcript: false },
      {},
      [
        "Upload audio",
        "Upload transcript",
        "Start listening",
        "Open in New Window",
        "Delete note",
      ],
    ],
    [
      "note with content",
      { transcript: false, content: "Existing content" },
      {},
      ["Start listening", "Open in New Window", "Delete note"],
    ],
    [
      "pending audio lookup",
      { transcript: false, audio: true, audioResolved: false },
      {},
      ["Resume listening", "Open in New Window", "Delete note"],
    ],
    [
      "transcript without a recording",
      {},
      {},
      ["Resume listening", "Open in New Window", "Delete note"],
    ],
    [
      "recorded audio",
      { transcript: false, audio: true },
      {},
      [
        "Resume listening",
        "Re-transcribe",
        "Show Audio in Finder",
        "Open in New Window",
        "Delete note",
      ],
    ],
    [
      "active listening",
      { transcript: false, mode: "active", floatingPanel: true },
      {},
      [
        "Start listening",
        "Open floating panel",
        "Open in New Window",
        "Delete note",
      ],
    ],
    [
      "finalizing",
      { audio: true, mode: "finalizing", floatingPanel: true },
      {},
      [
        "Resume listening",
        "Show Audio in Finder",
        "Open in New Window",
        "Delete note",
      ],
    ],
    [
      "batch transcription",
      { audio: true, mode: "running_batch" },
      {},
      [
        "Resume listening",
        "Show Audio in Finder",
        "Open in New Window",
        "Delete note",
      ],
    ],
    [
      "listening disabled",
      { mode: "active", floatingPanel: true },
      { allowListening: false },
      ["Open in New Window", "Delete note"],
    ],
    [
      "standalone window",
      {},
      { standaloneWindow: true },
      ["Resume listening", "Delete note"],
    ],
  ])("offers the right actions for %s", (_label, scenario, props, expected) => {
    arrange(scenario);
    renderOverflow(props);

    expect(visibleActions()).toEqual(expected);
  });

  it("uploads audio and transcripts into an empty note", () => {
    arrange({ transcript: false });
    renderOverflow();

    fireEvent.click(screen.getByRole("button", { name: "Upload audio" }));
    fireEvent.click(screen.getByRole("button", { name: "Upload transcript" }));

    expect(uploadAudioMock).toHaveBeenCalledTimes(1);
    expect(uploadTranscriptMock).toHaveBeenCalledTimes(1);
  });

  it("re-transcribes recorded audio", () => {
    arrange({ transcript: false, audio: true });
    renderOverflow();

    fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));

    expect(regenerateTranscriptMock).toHaveBeenCalledTimes(1);
  });

  it.each(["macos", "linux"])(
    "opens the floating panel while actively listening on %s",
    (platform) => {
      arrange({ mode: "active", floatingPanel: true, platform });
      renderOverflow();

      fireEvent.click(
        screen.getByRole("button", { name: "Open floating panel" }),
      );

      expect(openFloatingMeetingPanel).toHaveBeenCalledWith(
        expect.objectContaining({ sessionId: "session-1", enabled: true }),
      );
    },
  );

  it("opens the current note in a standalone window", () => {
    renderOverflow();

    fireEvent.click(screen.getByRole("button", { name: "Open in New Window" }));

    expect(windowShowMock).toHaveBeenCalledWith({
      type: "note",
      value: "session-1",
    });
  });

  it("opens the session folder for recorded audio", async () => {
    arrange({ transcript: false, audio: true });
    renderOverflow();

    fireEvent.click(
      screen.getByRole("button", { name: "Show Audio in Finder" }),
    );

    await waitFor(() => {
      expect(openPathMock).toHaveBeenCalledWith("/sessions/session-1", null);
    });
  });

  it("reports a recording deleted after the menu opened", async () => {
    arrange({ transcript: false, audio: true });
    audioExistMock.mockResolvedValue({ status: "ok", data: false });
    renderOverflow();

    fireEvent.click(
      screen.getByRole("button", { name: "Show Audio in Finder" }),
    );

    await waitFor(() => {
      expect(toastErrorMock).toHaveBeenCalled();
    });
    expect(openPathMock).not.toHaveBeenCalled();
  });

  it("requires authentication while the session record is still loading", async () => {
    arrange({ transcript: false, audio: true });
    sessionRecord.value = null;
    authenticateMock.mockResolvedValue(false);
    renderOverflow();

    fireEvent.click(
      screen.getByRole("button", { name: "Show Audio in Finder" }),
    );

    await waitFor(() => {
      expect(authenticateMock).toHaveBeenCalled();
    });
    expect(openPathMock).not.toHaveBeenCalled();
  });

  it("requires authentication for a locked note before opening the folder", async () => {
    arrange({ transcript: false, audio: true });
    sessionRecord.value = { locked: true };
    authenticateMock.mockResolvedValue(false);
    renderOverflow();

    fireEvent.click(
      screen.getByRole("button", { name: "Show Audio in Finder" }),
    );

    await waitFor(() => {
      expect(authenticateMock).toHaveBeenCalled();
    });
    expect(openPathMock).not.toHaveBeenCalled();
  });

  it("opens the export modal when export is selected", () => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 0;
    });
    renderOverflow();

    expect(exportModalMock).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    expect(exportModalMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ open: true }),
    );
  });

  it("remounts the export modal on every open", () => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 0;
    });
    renderOverflow();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    expect(exportModalMounts.value).toBe(1);

    const calls = exportModalMock.mock.calls;
    const lastProps = calls[calls.length - 1]?.[0];
    act(() => {
      lastProps?.onOpenChange(false);
    });

    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    expect(exportModalMounts.value).toBe(2);
  });
});
