import TaskItem from "@tiptap/extension-task-item";
import TaskList from "@tiptap/extension-task-list";
import { Markdown } from "@tiptap/markdown";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { useEffect, useMemo, useRef } from "react";

// Daily-note editor. Narrower than the full session NoteEditor in
// `apps/desktop/src/editor`: paragraphs, headings, lists, GFM tasks, inline
// formatting, and code. The contract is the CFM doc in `./cfm.md`.
//
// We round-trip through markdown because that's what the desktop2 IPC
// stores (`daily_notes.content: string`). `contentType: "markdown"` on
// mount + `editor.markdown.serialize` on change.

export type DailyNoteEditorProps = {
  // Canonical markdown value stored on disk.
  value: string;
  onChange: (next: string) => void;
  editable?: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  // Debounce interval for `onChange`. Defaults to 0 (synchronous) so the
  // surrounding container can own debouncing — most callers will want to.
  debounceMs?: number;
  className?: string;
};

export function DailyNoteEditor({
  value,
  onChange,
  editable = true,
  placeholder,
  autoFocus,
  debounceMs = 0,
  className,
}: DailyNoteEditorProps) {
  // Ref + debounce isolate us from React re-renders wiping the timer between
  // editor updates.
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const timerRef = useRef<number | null>(null);

  const extensions = useMemo(
    () => [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
        // We drop a few things that don't belong in a quick daily note:
        // blockquote/code-block are fine, but images/horizontal-rules are
        // noise for a textarea replacement. Users who need them can always
        // paste markdown directly.
        horizontalRule: false,
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Markdown,
    ],
    [],
  );

  const editor = useEditor(
    {
      extensions,
      editable,
      content: value,
      contentType: "markdown",
      autofocus: autoFocus ? "end" : false,
      immediatelyRender: false,
      shouldRerenderOnTransaction: false,
      onCreate: ({ editor }) => {
        editor.view.dom.setAttribute("spellcheck", "false");
      },
      onUpdate: ({ editor }) => {
        const next = editor.getMarkdown();

        if (debounceMs <= 0) {
          onChangeRef.current(next);
          return;
        }

        if (timerRef.current !== null) {
          window.clearTimeout(timerRef.current);
        }
        timerRef.current = window.setTimeout(() => {
          onChangeRef.current(next);
        }, debounceMs);
      },
      editorProps: {
        attributes: {
          class: "hypr-daily-note-editor",
          "data-placeholder": placeholder ?? "",
        },
      },
    },
    [extensions, editable],
  );

  // External value updates (e.g. live-query delta from another window) should
  // replace the editor content — but only when the editor is unfocused, so
  // we don't stomp the user's in-progress typing.
  useEffect(() => {
    if (!editor) {
      return;
    }
    if (editor.getMarkdown() === value) {
      return;
    }
    if (editor.isFocused) {
      return;
    }
    editor.commands.setContent(value, {
      contentType: "markdown",
      emitUpdate: false,
    });
  }, [editor, value]);

  useEffect(
    () => () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
      }
    },
    [],
  );

  return <EditorContent editor={editor} className={className} />;
}
