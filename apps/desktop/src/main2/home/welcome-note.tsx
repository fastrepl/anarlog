import "./note-editor.css";

import { useCallback, useRef } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";

import {
  type JSONContent,
  NoteEditor,
  type NoteEditorRef,
} from "~/editor/session";
import * as main from "~/store/tinybase/store/main";

const WELCOME_ROW_ID = "welcome";

const welcomeContent: JSONContent = {
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: "This is your daily notes" }],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          text: "Every day you get a fresh note for your task, ideas and meetings \u2014 write it any way you want and build your own workflow. Char takes all context around your work and help you to fill your day.",
        },
      ],
    },
    {
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: "Create lists, tasks and mentions" }],
    },
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: "You can create todo lists there. Try to type ",
                },
                { type: "text", marks: [{ type: "code" }], text: "-[]" },
                { type: "text", text: " or just use " },
                { type: "text", marks: [{ type: "code" }], text: "/" },
                { type: "text", text: " for command" },
              ],
            },
          ],
        },
      ],
    },
    {
      type: "bulletList",
      content: [
        {
          type: "listItem",
          content: [
            {
              type: "paragraph",
              content: [
                { type: "text", text: "Write bullet lists and numbered one." },
              ],
            },
          ],
        },
      ],
    },
    {
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: "Check your meetings there" }],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          text: "All meetings from calendar appear in your daily notes. You can write notes inside the meetings or create new note",
        },
      ],
    },
    {
      type: "session",
      attrs: { sessionId: "welcome-demo", status: null, checked: null },
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "This is dummy meeting" }],
        },
      ],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          marks: [{ type: "italic" }],
          text: "Create a new note by pressing ",
        },
        {
          type: "text",
          marks: [{ type: "code" }],
          text: "new recording",
        },
        {
          type: "text",
          marks: [{ type: "italic" }],
          text: " button",
        },
      ],
    },
    {
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: "Make this fully yourself with AI" }],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          text: "Get the tasks done right from Daily Notes with multiple ",
        },
        {
          type: "text",
          marks: [
            {
              type: "link",
              attrs: { href: "https://char.com/docs/integrations" },
            },
          ],
          text: "integrations",
        },
        { type: "text", text: " Char has" },
      ],
    },
    {
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: "And much more" }],
    },
    {
      type: "paragraph",
      content: [
        { type: "text", text: "Read " },
        {
          type: "text",
          marks: [{ type: "link", attrs: { href: "https://char.com/docs" } }],
          text: "docs",
        },
        {
          type: "text",
          text: " to integrate all features in your workflow. Join us on ",
        },
        {
          type: "text",
          marks: [{ type: "link", attrs: { href: "https://x.com/char" } }],
          text: "X",
        },
        { type: "text", text: " or " },
        {
          type: "text",
          marks: [{ type: "link", attrs: { href: "https://discord.gg/char" } }],
          text: "Discord",
        },
        { type: "text", text: " to get the last updates" },
      ],
    },
    {
      type: "heading",
      attrs: { level: 2 },
      content: [
        { type: "text", text: "All your previous notes perfectly fine" },
      ],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          text: "They are available through previous Daily Notes, search and folder where you keep them, nothing changes",
        },
      ],
    },
  ],
};

function readSavedContent(
  store: NonNullable<ReturnType<typeof main.UI.useStore>>,
): JSONContent | null {
  const cell = store.getCell("daily_notes", WELCOME_ROW_ID, "content");
  if (typeof cell !== "string" || cell === "") return null;
  return parseJsonContent(cell) ?? null;
}

export function WelcomeNote({ onDismiss }: { onDismiss: () => void }) {
  const store = main.UI.useStore(main.STORE_ID);
  const editorRef = useRef<NoteEditorRef>(null);

  const initialContentRef = useRef<JSONContent | null>(null);
  if (!initialContentRef.current && store) {
    initialContentRef.current = readSavedContent(store) ?? welcomeContent;
  }

  const persistWelcomeNote = main.UI.useSetPartialRowCallback(
    "daily_notes",
    WELCOME_ROW_ID,
    (input: JSONContent) => ({
      content: JSON.stringify(input),
      date: WELCOME_ROW_ID,
    }),
    [],
    main.STORE_ID,
  );

  const handleChange = useCallback(
    (input: JSONContent) => {
      persistWelcomeNote(input);
    },
    [persistWelcomeNote],
  );

  if (!initialContentRef.current) {
    return null;
  }

  return (
    <div>
      <div className="flex items-center gap-3 px-6 pt-6 pb-3">
        <h2 className="text-xl font-semibold text-neutral-900">
          Welcome to Char
        </h2>
      </div>

      <div className="main2-daily-note-editor px-6">
        <NoteEditor
          ref={editorRef}
          key="daily-welcome"
          initialContent={initialContentRef.current}
          handleChange={handleChange}
          linkedItemOpenBehavior="new"
        />
      </div>

      <div className="px-6 pt-4 pb-6">
        <button
          onClick={onDismiss}
          className="rounded-full bg-neutral-900 px-5 py-2.5 text-sm font-medium text-white transition-colors hover:bg-neutral-800"
        >
          Start to use your Char
        </button>
      </div>
    </div>
  );
}
