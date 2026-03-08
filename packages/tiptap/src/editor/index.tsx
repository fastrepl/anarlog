import "../../styles.css";

import { isNodeActive } from "@tiptap/core";
import {
  EditorContent,
  type JSONContent,
  type Editor as TiptapEditor,
  useEditor,
} from "@tiptap/react";
import { forwardRef, useEffect, useMemo, useRef } from "react";
import { useDebounceCallback } from "usehooks-ts";

import * as shared from "../shared";
import type { ExtensionOptions, FileHandlerConfig } from "../shared/extensions";
import type { PlaceholderFunction } from "../shared/extensions/placeholder";
import { isMentionActive, mention, type MentionConfig } from "./mention";

const safeRequestIdleCallback =
  typeof requestIdleCallback !== "undefined"
    ? requestIdleCallback
    : (cb: IdleRequestCallback) =>
        setTimeout(
          () =>
            cb({
              didTimeout: false,
              timeRemaining: () => 50,
            } as IdleDeadline),
          1,
        );

export type { JSONContent, TiptapEditor };

interface EditorProps {
  handleChange?: (content: JSONContent) => void;
  initialContent?: JSONContent;
  editable?: boolean;
  setContentFromOutside?: boolean;
  mentionConfig?: MentionConfig;
  placeholderComponent?: PlaceholderFunction;
  fileHandlerConfig?: FileHandlerConfig;
  extensionOptions?: ExtensionOptions;
  onNavigateToTitle?: () => void;
}

const Editor = forwardRef<{ editor: TiptapEditor | null }, EditorProps>(
  (props, ref) => {
    const {
      handleChange,
      initialContent,
      editable = true,
      setContentFromOutside = false,
      mentionConfig,
      placeholderComponent,
      fileHandlerConfig,
      extensionOptions,
      onNavigateToTitle,
    } = props;
    const previousContentRef = useRef<JSONContent>(initialContent);

    const onUpdate = useDebounceCallback(
      ({ editor }: { editor: TiptapEditor }) => {
        if (!editor.isInitialized || !handleChange) {
          return;
        }

        safeRequestIdleCallback(() => {
          const content = editor.getJSON();
          handleChange(content);
        });
      },
      500,
    );

    const extensions = useMemo(
      () => [
        ...shared.getExtensions(
          placeholderComponent,
          fileHandlerConfig,
          extensionOptions,
        ),
        ...(mentionConfig ? [mention(mentionConfig)] : []),
      ],
      [
        mentionConfig,
        placeholderComponent,
        fileHandlerConfig,
        extensionOptions,
      ],
    );

    const editorProps: Parameters<typeof useEditor>[0]["editorProps"] = useMemo(
      () => ({
        scrollThreshold: 32,
        scrollMargin: 32,
        handleKeyDown: (view, event) => {
          const allowedGlobalShortcuts = ["w", "n", "t", ",", "j", "l", "k"];
          if (
            (event.metaKey || event.ctrlKey) &&
            allowedGlobalShortcuts.includes(event.key)
          ) {
            return false;
          }

          const { state } = view;
          const isAtStart = state.selection.$head.pos === 0;

          const $head = state.selection.$head;
          let isInFirstBlock = false;

          let node = state.doc.firstChild;
          let firstTextBlockPos = 0;
          while (node && !node.isTextblock) {
            firstTextBlockPos += 1;
            node = node.firstChild;
          }

          if (node) {
            isInFirstBlock = $head.start($head.depth) === firstTextBlockPos + 1;
          }

          if (
            event.key === "ArrowUp" &&
            isInFirstBlock &&
            onNavigateToTitle &&
            !isMentionActive(state)
          ) {
            event.preventDefault();

            const firstBlock = state.doc.firstChild;
            if (firstBlock && firstBlock.textContent) {
              const text = firstBlock.textContent;
              const posInBlock = $head.pos - $head.start();
              const textBeforeCursor = text.slice(0, posInBlock);

              const editorDom = view.dom;
              const firstTextNode = editorDom.querySelector(".ProseMirror > *");

              if (firstTextNode) {
                const editorStyle = window.getComputedStyle(firstTextNode);
                const canvas = document.createElement("canvas");
                const ctx = canvas.getContext("2d");

                if (ctx) {
                  ctx.font = `${editorStyle.fontWeight} ${editorStyle.fontSize} ${editorStyle.fontFamily}`;
                  const editorWidth = ctx.measureText(textBeforeCursor).width;

                  setTimeout(() => {
                    const navEvent = new CustomEvent(
                      "editor-move-to-title-position",
                      {
                        detail: { pixelWidth: editorWidth },
                      },
                    );
                    window.dispatchEvent(navEvent);
                  }, 0);
                }
              }
            }

            onNavigateToTitle();
            return true;
          }

          if (event.key === "Tab" && event.shiftKey) {
            const isInListItem =
              isNodeActive(state, "listItem") ||
              isNodeActive(state, "taskItem");
            if (!isInListItem && isInFirstBlock && onNavigateToTitle) {
              event.preventDefault();
              onNavigateToTitle();
              return true;
            }
          }

          if (event.key === "Backspace") {
            if (isAtStart && state.selection.empty) {
              event.preventDefault();
              return true;
            }
          }

          if (event.key === "Tab") {
            const isInListItem =
              isNodeActive(state, "listItem") ||
              isNodeActive(state, "taskItem");
            if (isInListItem) {
              return false;
            }
            event.preventDefault();
            return true;
          }

          return false;
        },
      }),
      [onNavigateToTitle],
    );

    const editor = useEditor(
      {
        extensions,
        editable,
        content: shared.isValidTiptapContent(initialContent)
          ? initialContent
          : shared.EMPTY_TIPTAP_DOC,
        onCreate: ({ editor }) => {
          editor.view.dom.setAttribute("spellcheck", "false");
          editor.view.dom.setAttribute("autocomplete", "off");
          editor.view.dom.setAttribute("autocorrect", "off");
          editor.view.dom.setAttribute("autocapitalize", "off");
        },
        onUpdate,
        immediatelyRender: false,
        shouldRerenderOnTransaction: false,
        parseOptions: { preserveWhitespace: "full" },
        editorProps,
      },
      [extensions],
    );

    useEffect(() => {
      if (ref && typeof ref === "object") {
        ref.current = { editor };
      }
    }, [editor]);

    useEffect(() => {
      if (
        editor &&
        (setContentFromOutside || previousContentRef.current !== initialContent)
      ) {
        previousContentRef.current = initialContent;
        if (setContentFromOutside) {
          const { from, to } = editor.state.selection;
          if (shared.isValidTiptapContent(initialContent)) {
            editor.commands.markNewContent();
          }

          if (from > 0 && to > 0 && from < editor.state.doc.content.size) {
            editor.commands.setTextSelection({ from, to });
          }
        } else if (!editor.isFocused) {
          if (shared.isValidTiptapContent(initialContent)) {
            editor.commands.setContent(initialContent, {
              parseOptions: { preserveWhitespace: "full" },
            });
          }
        }
      }
    }, [editor, initialContent, setContentFromOutside]);

    useEffect(() => {
      if (editor) {
        editor.setEditable(editable);
      }
    }, [editor, editable]);

    useEffect(() => {
      const platform = navigator.platform.toLowerCase();
      if (platform.includes("win")) {
        document.body.classList.add("platform-windows");
      } else if (platform.includes("linux")) {
        document.body.classList.add("platform-linux");
      }

      return () => {
        document.body.classList.remove("platform-windows", "platform-linux");
      };
    }, []);

    return (
      <EditorContent editor={editor} className="tiptap-root" role="textbox" />
    );
  },
);

Editor.displayName = "Editor";

export default Editor;
