import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EnhancedEditor } from "./editor";

const hoisted = vi.hoisted(() => ({
  content: JSON.stringify({ type: "doc", content: [] }),
  handleChange: vi.fn(),
  noteEditorProps: [] as Record<string, unknown>[],
}));

vi.mock("@hypr/editor/markdown", () => ({
  parseJsonContent: (value: string) => JSON.parse(value),
}));

vi.mock("@hypr/editor/note", () => ({
  NoteEditor: (props: Record<string, unknown>) => {
    hoisted.noteEditorProps.push(props);

    return <div>Note editor</div>;
  },
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

vi.mock("~/shared/hooks/useFileUpload", () => ({
  useFileUpload: () => vi.fn(),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: () => hoisted.content,
    useSetPartialRowCallback: () => hoisted.handleChange,
  },
}));

describe("EnhancedEditor", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.noteEditorProps = [];
    hoisted.content = JSON.stringify({ type: "doc", content: [] });
    hoisted.handleChange = vi.fn();
  });

  it("keeps persisted notes from syncing external content while focused", () => {
    render(<EnhancedEditor sessionId="session-1" enhancedNoteId="note-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];

    expect(props?.syncContentWhenFocused).toBe(false);
    expect(props?.handleChange).toBe(hoisted.handleChange);
    expect(props?.taskSource).toEqual({ type: "enhanced_note", id: "note-1" });
  });

  it("keeps streamed previews syncing while focused", () => {
    const contentOverride = {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "Generating" }] },
      ],
    };

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        contentOverride={contentOverride}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];

    expect(props?.syncContentWhenFocused).toBe(true);
    expect(props?.handleChange).toBeUndefined();
    expect(props?.taskSource).toBeUndefined();
    expect(props?.initialContent).toBe(contentOverride);
  });
});
