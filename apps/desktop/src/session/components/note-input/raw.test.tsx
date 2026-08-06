import {
  cleanup,
  createEvent,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { RawEditor as SessionRawEditor } from "./raw";

const hoisted = vi.hoisted(() => ({
  rawMd: JSON.stringify({ type: "doc", content: [] }),
  sessionTitle: "Weekly sync",
  persistChange: vi.fn(() => Promise.resolve()),
  fileUpload: vi.fn(),
  processAudioFile: vi.fn(),
  showWindow: vi.fn(),
  unminimizeWindow: vi.fn(),
  focusWindow: vi.fn(),
  meetingChatRecords: [] as unknown[],
  userTemplates: [] as Array<Record<string, unknown>>,
  noteEditorProps: [] as Record<string, unknown>[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    show: hoisted.showWindow,
    unminimize: hoisted.unminimizeWindow,
    setFocus: hoisted.focusWindow,
  }),
}));

vi.mock("@anlg/editor/markdown", () => ({
  parseJsonContent: (value: string) => JSON.parse(value),
}));

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    t: (input: TemplateStringsArray) => input.join(""),
  }),
}));

vi.mock("@anlg/editor/note", () => ({
  normalizePortableAttachmentUrls: (value: unknown) => value,
  NoteEditor: (props: Record<string, unknown>) => {
    hoisted.noteEditorProps.push(props);

    return <div>Note editor</div>;
  },
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    event: vi.fn(),
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: vi.fn() },
}));

vi.mock("~/editor-bridge/app-link-view", () => ({
  AppLinkView: () => null,
}));

vi.mock("~/editor-bridge/mention-config", () => ({
  useMentionConfig: () => ({ users: [] }),
}));

vi.mock("~/editor-bridge/open-editor-link", () => ({
  openEditorLink: vi.fn(),
}));

vi.mock("~/editor-bridge/session-mention-drop", () => ({
  sessionMentionDropConfig: { read: () => null },
}));

vi.mock("~/editor-bridge/session-view", () => ({
  SessionNodeView: () => null,
}));

vi.mock("~/session-sharing/comment-anchors", () => ({
  useSessionCommentAnchors: () => ({
    onViewReady: vi.fn(),
    onViewDisposed: vi.fn(),
  }),
}));

vi.mock("~/session/components/shared", () => ({
  hasStoredNoteContent: (value: unknown) => {
    if (typeof value !== "string" || !value.trim()) {
      return false;
    }

    try {
      const hasText = (node: unknown): boolean => {
        if (!node || typeof node !== "object") {
          return false;
        }
        const content = node as {
          text?: string;
          content?: unknown[];
        };
        return (
          Boolean(content.text?.trim()) ||
          Boolean(content.content?.some(hasText))
        );
      };

      return hasText(JSON.parse(value));
    } catch {
      return true;
    }
  },
}));

vi.mock("~/session/queries", () => ({
  useUpdateSession: () => hoisted.persistChange,
}));

vi.mock("~/session/hooks/useAttachmentResolver", () => ({
  useAttachmentResolver: () => () => null,
}));

vi.mock("~/templates", () => ({
  TemplateIconGlyph: () => <span aria-hidden>Template icon</span>,
  useUserTemplates: () => hoisted.userTemplates,
}));

function RawEditor({
  sessionId,
  className,
}: {
  sessionId: string;
  className?: string;
}) {
  return (
    <SessionRawEditor
      sessionId={sessionId}
      rawMd={hoisted.rawMd}
      sessionTitle={hoisted.sessionTitle}
      className={className}
    />
  );
}

vi.mock("~/stt/meeting-chat-records", () => ({
  formatMeetingPlatform: (platform: string) =>
    ({
      zoom: "Zoom",
      googleMeet: "Google Meet",
      microsoftTeams: "Microsoft Teams",
      slack: "Slack",
      discord: "Discord",
      webex: "Webex",
      unknown: "Meeting app",
    })[platform] ?? "Meeting app",
  useMeetingChatRecords: () => hoisted.meetingChatRecords,
}));

vi.mock("~/shared/hooks/useFileUpload", () => ({
  useFileUpload: () => hoisted.fileUpload,
}));

vi.mock("~/stt/useUploadFile", () => ({
  AUDIO_EXTENSIONS: ["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"],
  isAudioUploadFile: (file: Pick<File, "name" | "type">) =>
    file.type.startsWith("audio/") ||
    ["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac", "qta"].some(
      (extension) => file.name.endsWith(`.${extension}`),
    ),
  useUploadFile: () => ({ processAudioFile: hoisted.processAudioFile }),
}));

describe("RawEditor", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.noteEditorProps = [];
    hoisted.rawMd = JSON.stringify({ type: "doc", content: [] });
    hoisted.sessionTitle = "Weekly sync";
    hoisted.persistChange = vi.fn(() => Promise.resolve());
    hoisted.fileUpload = vi.fn();
    hoisted.processAudioFile = vi.fn();
    hoisted.meetingChatRecords = [];
    hoisted.userTemplates = [];
    hoisted.showWindow.mockReset();
    hoisted.unminimizeWindow.mockReset();
    hoisted.focusWindow.mockReset();
    hoisted.showWindow.mockResolvedValue(undefined);
    hoisted.unminimizeWindow.mockResolvedValue(undefined);
    hoisted.focusWindow.mockResolvedValue(undefined);
  });

  it("uses the shared session note editor styling", () => {
    render(<RawEditor sessionId="session-1" className="custom-editor-class" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];

    expect(props?.className).toContain("session-note-editor");
    expect(props?.className).toContain("custom-editor-class");
    expect(props?.placeholderComponent).toEqual(expect.any(Function));
    expect((props?.placeholderComponent as () => string)()).toBe(
      "Start writing...",
    );
    expect(props?.persistentPlaceholderComponent).toBeUndefined();
    expect(props?.enforceTitleHeading).toBe(false);
    expect(props?.initialContent).toMatchObject({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
  });

  it("removes a legacy session title from the memo body", () => {
    hoisted.rawMd = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    });

    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    expect(props?.initialContent).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    });
  });

  it("persists memo content without changing the session title", async () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const content = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
      ],
    };
    (props?.handleChange as (content: unknown) => void)(content);

    await waitFor(() =>
      expect(hoisted.persistChange).toHaveBeenCalledWith({
        raw_md: JSON.stringify(content),
      }),
    );
  });

  it("applies favorite template sections to an empty memo", async () => {
    hoisted.userTemplates = [
      {
        id: "template-unpinned",
        title: "Project Kickoff",
        pinned: false,
        pinOrder: null,
        icon: { type: "emoji", value: "🚀" },
        sections: [{ title: "Goals", description: "" }],
      },
      {
        id: "template-retro",
        title: "Retrospective",
        pinned: true,
        pinOrder: 2,
        icon: { type: "emoji", value: "🔁" },
        sections: [{ title: "What we learned", description: "" }],
      },
      {
        id: "template-standup",
        title: "Standup",
        pinned: true,
        pinOrder: 1,
        icon: { type: "emoji", value: "☀️" },
        sections: [
          { title: " Yesterday ", description: "" },
          { title: "Today", description: "" },
        ],
      },
    ];

    render(<RawEditor sessionId="session-1" />);

    expect(
      screen.getAllByRole("button").map((button) => button.textContent),
    ).toEqual(["Template iconStandup", "Template iconRetrospective"]);
    expect(
      screen.queryByRole("button", { name: "Project Kickoff" }),
    ).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Standup" }));

    await waitFor(() =>
      expect(hoisted.persistChange).toHaveBeenCalledWith({
        raw_md: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Yesterday" }],
            },
            { type: "paragraph" },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Today" }],
            },
            { type: "paragraph" },
          ],
        }),
      }),
    );
  });

  it("hides favorite templates when the memo has content", () => {
    hoisted.rawMd = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Existing memo" }],
        },
      ],
    });
    hoisted.userTemplates = [
      {
        id: "template-standup",
        title: "Standup",
        pinned: true,
        pinOrder: 1,
        icon: { type: "emoji", value: "☀️" },
        sections: [{ title: "Today", description: "" }],
      },
    ];

    render(<RawEditor sessionId="session-1" />);

    expect(screen.queryByRole("button", { name: "Standup" })).toBeNull();
  });

  it("renders captured chat without mutating the active memo editor", () => {
    const { rerender } = render(<RawEditor sessionId="session-1" />);
    hoisted.meetingChatRecords = [
      {
        id: "msg-1",
        platform: "zoom",
        surface: "native",
        sender: "Ada",
        timestamp: "10:42 AM",
        direction: "incoming",
        text: "Review this together",
        links: [],
        capturedAt: "2026-07-13T10:00:00.000Z",
      },
    ];

    rerender(<RawEditor sessionId="session-1" />);

    expect(screen.getByText("Review this together")).not.toBeNull();
    expect(hoisted.persistChange).not.toHaveBeenCalled();
  });

  it("routes dropped audio files to transcription", () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onDrop: (
        files: File[],
        pos?: number,
        items?: DataTransferItemList,
      ) => boolean | void | { remainingFiles: File[] };
    };
    const file = { name: "clip.mp3", type: "audio/mpeg" } as File;

    expect(fileHandlerConfig.onDrop([file])).toBe(true);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file);
  });

  it("keeps non-audio files available when audio is dropped with attachments", () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onDrop: (files: File[]) => boolean | void | { remainingFiles: File[] };
    };
    const audioFile = { name: "clip.mp3", type: "audio/mpeg" } as File;
    const imageFile = { name: "photo.png", type: "image/png" } as File;

    expect(fileHandlerConfig.onDrop([audioFile, imageFile])).toEqual({
      remainingFiles: [imageFile],
    });
    expect(hoisted.processAudioFile).toHaveBeenCalledTimes(1);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(audioFile);
  });

  it("uses drag item MIME for mixed drops handled by the editor", () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onDrop: (
        files: File[],
        pos?: number,
        items?: DataTransferItemList,
      ) => boolean | void | { remainingFiles: File[] };
    };
    const audioFile = new File(["audio"], "clip", { type: "" });
    const imageFile = new File(["image"], "photo.png", { type: "image/png" });
    const dataTransfer = audioDataTransfer(
      [audioFile, imageFile],
      ["audio/mpeg", imageFile.type],
    );

    expect(
      fileHandlerConfig.onDrop(
        [audioFile, imageFile],
        undefined,
        dataTransfer.items,
      ),
    ).toEqual({
      remainingFiles: [imageFile],
    });
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(audioFile, {
      allowUnknownAudio: true,
      contentType: "audio/mpeg",
    });
  });

  it("routes pasted Voice Memos audio to transcription", () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onPaste: (
        files: File[],
        items?: DataTransferItemList,
      ) => boolean | void | { remainingFiles: File[] };
    };
    const file = new File(["audio"], "Brian Shin.qta", { type: "" });
    const dataTransfer = audioDataTransfer(file, "audio/quicktime");

    expect(fileHandlerConfig.onPaste([file], dataTransfer.items)).toBe(true);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file);
  });

  it("only imports the first audio file from a multi-audio drop", () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onDrop: (files: File[]) => boolean | void | { remainingFiles: File[] };
    };
    const firstAudioFile = { name: "first.mp3", type: "audio/mpeg" } as File;
    const secondAudioFile = { name: "second.m4a", type: "" } as File;

    expect(fileHandlerConfig.onDrop([firstAudioFile, secondAudioFile])).toEqual(
      {
        remainingFiles: [secondAudioFile],
      },
    );
    expect(hoisted.processAudioFile).toHaveBeenCalledTimes(1);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(firstAudioFile);
  });

  it("shows an audio upload overlay and intercepts audio drops", async () => {
    render(<RawEditor sessionId="session-1" />);

    const file = new File(["audio"], "clip.flac", { type: "" });
    const dataTransfer = audioDataTransfer(file);
    const dropTarget = screen.getByText("Note editor").parentElement;

    expect(dropTarget).not.toBeNull();
    fireEvent.dragEnter(dropTarget!, { dataTransfer });

    expect(
      screen.getByText("Drop to upload and transcribe audio"),
    ).not.toBeNull();
    expect(screen.getByRole("status").className).not.toContain("backdrop-blur");
    expect(
      screen.getByText("WAV, MP3, OGG, MP4, M4A, FLAC, WEBM, or AAC audio"),
    ).not.toBeNull();
    await waitFor(() => expect(hoisted.focusWindow).toHaveBeenCalledTimes(1));
    expect(hoisted.showWindow).toHaveBeenCalledTimes(1);
    expect(hoisted.unminimizeWindow).toHaveBeenCalledTimes(1);

    fireEvent.drop(dropTarget!, { dataTransfer });

    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file);
    expect(
      screen.queryByText("Drop to upload and transcribe audio"),
    ).toBeNull();
  });

  it("does not capture mixed audio and attachment drops on the wrapper", () => {
    render(<RawEditor sessionId="session-1" />);

    const audioFile = new File(["audio"], "clip.mp3", { type: "audio/mpeg" });
    const imageFile = new File(["image"], "photo.png", { type: "image/png" });
    const dataTransfer = audioDataTransfer([audioFile, imageFile]);
    const dropTarget = screen.getByText("Note editor").parentElement;

    expect(dropTarget).not.toBeNull();
    const dropEvent = createEvent.drop(dropTarget!, { dataTransfer });
    fireEvent(dropTarget!, dropEvent);

    expect(dropEvent.defaultPrevented).toBe(false);
    expect(hoisted.processAudioFile).not.toHaveBeenCalled();
  });

  it("uses the drag item MIME when dropped audio has no MIME or extension", async () => {
    render(<RawEditor sessionId="session-1" />);

    const file = new File(["audio"], "clip", { type: "" });
    const dataTransfer = audioDataTransfer(file, "audio/mpeg");
    const dropTarget = screen.getByText("Note editor").parentElement;

    expect(dropTarget).not.toBeNull();
    fireEvent.dragEnter(dropTarget!, { dataTransfer });

    expect(
      screen.getByText("Drop to upload and transcribe audio"),
    ).not.toBeNull();

    fireEvent.drop(dropTarget!, { dataTransfer });

    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file, {
      allowUnknownAudio: true,
      contentType: "audio/mpeg",
    });
  });
});

function audioDataTransfer(input: File | File[], itemType?: string | string[]) {
  const files = Array.isArray(input) ? input : [input];
  const itemTypes = Array.isArray(itemType)
    ? itemType
    : files.map((file) => itemType ?? file.type);

  return {
    files,
    items: files.map((file, index) => ({
      kind: "file",
      type: itemTypes[index] ?? file.type,
      getAsFile: () => file,
    })),
    types: ["Files"],
    dropEffect: "none",
  } as unknown as DataTransfer;
}
