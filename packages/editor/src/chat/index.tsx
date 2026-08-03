import "prosemirror-view/style/prosemirror.css";
import "../styles/prosemirror.css";

import {
  ProseMirror,
  ProseMirrorDoc,
  reactKeys,
  useEditorEffect,
} from "@handlewithcare/react-prosemirror";
import {
  chainCommands,
  createParagraphNear,
  deleteSelection,
  joinBackward,
  joinForward,
  liftEmptyBlock,
  selectAll,
  selectNodeBackward,
  selectNodeForward,
  splitBlock,
} from "prosemirror-commands";
import { history, redo, undo } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { Node as PMNode } from "prosemirror-model";
import { EditorState, Plugin, PluginKey, Selection } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { forwardRef, useImperativeHandle, useMemo, useRef } from "react";

import { cn } from "@anlg/utils";

import { EditorErrorBoundary } from "../editor-error-boundary";
import {
  AttachmentChipView,
  MentionNodeView,
  withNodeViewErrorBoundary,
} from "../node-views";
import {
  docChangeListenerPlugin,
  type PlaceholderFunction,
  placeholderPlugin,
} from "../plugins";
import {
  type MentionConfig,
  MentionSuggestion,
  findMention,
  mentionSkipPlugin,
} from "../widgets";
import { chatSchema } from "./schema";

export { chatSchema };
export type { MentionConfig };

export interface JSONContent {
  type?: string;
  attrs?: Record<string, any>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, any> }[];
  text?: string;
}

export interface ChatEditorHandle {
  focus(): boolean;
  getJSON(): JSONContent | undefined;
  clearContent(): void;
  replaceContent(content: JSONContent, selection?: "start" | "end"): void;
}

interface ChatEditorProps {
  className?: string;
  initialContent?: JSONContent;
  mentionConfig?: MentionConfig;
  placeholder?: PlaceholderFunction;
  submitShortcut?: "mod-enter" | "enter";
  onUpdate?: (json: JSONContent) => void;
  onSubmit?: () => void;
  onHistoryNavigate?: (direction: "prev" | "next") => boolean;
}

const nodeViews = {
  "mention-@": withNodeViewErrorBoundary<HTMLElement>(MentionNodeView, {
    name: "mention-@",
  }),
  attachment: withNodeViewErrorBoundary<HTMLSpanElement>(AttachmentChipView, {
    name: "attachment",
  }),
};

function ViewCapture({
  viewRef,
}: {
  viewRef: React.RefObject<EditorView | null>;
}) {
  useEditorEffect((view) => {
    if (view && viewRef.current !== view) {
      viewRef.current = view;
    }
  });
  return null;
}

const mac =
  typeof navigator !== "undefined"
    ? /Mac|iP(hone|[oa]d)/.test(navigator.platform)
    : false;

function fileHandlerPlugin() {
  return new Plugin({
    key: new PluginKey("chatFileHandler"),
    props: {
      handleDrop(view, event) {
        const files = Array.from(event.dataTransfer?.files ?? []);
        if (files.length === 0) return false;
        event.preventDefault();
        insertFiles(view, files);
        return true;
      },
      handlePaste(view, event) {
        const files = Array.from(event.clipboardData?.files ?? []);
        if (files.length === 0) return false;
        insertFiles(view, files);
        return true;
      },
    },
  });
}

function insertFiles(view: EditorView, files: File[]) {
  for (const file of files) {
    if (file.type.startsWith("image/")) {
      const reader = new FileReader();
      reader.readAsDataURL(file);
      reader.onload = () => {
        insertAttachmentNode(view, {
          id: crypto.randomUUID(),
          name: file.name,
          mimeType: file.type,
          url: reader.result as string,
          size: file.size,
        });
      };
    } else {
      insertAttachmentNode(view, {
        id: crypto.randomUUID(),
        name: file.name,
        mimeType: file.type,
        url: null,
        size: file.size,
      });
    }
  }
}

function insertAttachmentNode(
  view: EditorView,
  attrs: {
    id: string;
    name: string;
    mimeType: string;
    url: string | null;
    size: number;
  },
) {
  const { schema } = view.state;
  const node = schema.nodes.attachment.create(attrs);
  const space = schema.text(" ");
  const { from, to } = view.state.selection;
  const tr = view.state.tr.replaceWith(from, to, [node, space]);
  view.dispatch(tr);
  view.focus();
}

export const ChatEditor = forwardRef<ChatEditorHandle, ChatEditorProps>(
  function ChatEditor(props, ref) {
    const {
      className,
      initialContent,
      mentionConfig,
      placeholder,
      submitShortcut = "mod-enter",
      onUpdate,
      onSubmit,
      onHistoryNavigate,
    } = props;

    const viewRef = useRef<EditorView | null>(null);
    const onSubmitRef = useRef(onSubmit);
    onSubmitRef.current = onSubmit;
    const onUpdateRef = useRef(onUpdate);
    onUpdateRef.current = onUpdate;
    const onHistoryNavigateRef = useRef(onHistoryNavigate);
    onHistoryNavigateRef.current = onHistoryNavigate;

    useImperativeHandle(
      ref,
      () => ({
        focus() {
          const view = viewRef.current;
          if (!view) return false;

          view.focus();
          return true;
        },
        getJSON() {
          return viewRef.current?.state.doc.toJSON() as JSONContent | undefined;
        },
        clearContent() {
          const view = viewRef.current;
          if (!view) return;
          const doc = chatSchema.node("doc", null, [
            chatSchema.node("paragraph"),
          ]);
          const tr = view.state.tr.replaceWith(
            0,
            view.state.doc.content.size,
            doc.content,
          );
          view.dispatch(tr);
        },
        replaceContent(content, selection = "end") {
          const view = viewRef.current;
          if (!view || content.type !== "doc") return;

          let doc: PMNode;
          try {
            doc = PMNode.fromJSON(chatSchema, content);
          } catch {
            return;
          }

          const tr = view.state.tr.replaceWith(
            0,
            view.state.doc.content.size,
            doc.content,
          );
          tr.setSelection(
            selection === "start"
              ? Selection.atStart(tr.doc)
              : Selection.atEnd(tr.doc),
          );
          view.dispatch(tr);
          view.focus();
        },
      }),
      [],
    );

    const plugins = useMemo(() => {
      const submitCommand = (state: EditorState) => {
        if (mentionConfig && findMention(state, mentionConfig.trigger)) {
          return false;
        }
        onSubmitRef.current?.();
        return true;
      };
      const enterCommand =
        submitShortcut === "enter"
          ? submitCommand
          : chainCommands(createParagraphNear, liftEmptyBlock, splitBlock);
      const shiftEnterCommand =
        submitShortcut === "enter"
          ? chainCommands(createParagraphNear, liftEmptyBlock, splitBlock)
          : undefined;
      const historyNavCommand =
        (direction: "prev" | "next") => (state: EditorState) => {
          if (!state.selection.empty) {
            return false;
          }
          if (mentionConfig && findMention(state, mentionConfig.trigger)) {
            return false;
          }

          const edge =
            direction === "prev"
              ? Selection.atStart(state.doc).from
              : Selection.atEnd(state.doc).to;
          if (state.selection.from !== edge) {
            return false;
          }

          return onHistoryNavigateRef.current?.(direction) ?? false;
        };

      return [
        reactKeys(),
        docChangeListenerPlugin((doc) => {
          onUpdateRef.current?.(doc.toJSON() as JSONContent);
        }),
        keymap({
          "Mod-z": undo,
          "Mod-Shift-z": redo,
          ...(!mac ? { "Mod-y": redo } : {}),
          ...(submitShortcut === "mod-enter"
            ? { "Mod-Enter": submitCommand }
            : {}),
          ...(shiftEnterCommand ? { "Shift-Enter": shiftEnterCommand } : {}),
          Enter: enterCommand,
          Backspace: chainCommands(
            deleteSelection,
            joinBackward,
            selectNodeBackward,
          ),
          Delete: chainCommands(
            deleteSelection,
            joinForward,
            selectNodeForward,
          ),
          "Mod-a": selectAll,
          ArrowUp: historyNavCommand("prev"),
          ArrowDown: historyNavCommand("next"),
        }),
        history(),
        placeholderPlugin(placeholder),
        ...(mentionConfig ? [mentionSkipPlugin()] : []),
        fileHandlerPlugin(),
      ];
    }, [mentionConfig, placeholder, submitShortcut]);

    const defaultState = useMemo(() => {
      let doc: PMNode;
      try {
        doc =
          initialContent && initialContent.type === "doc"
            ? PMNode.fromJSON(chatSchema, initialContent)
            : chatSchema.node("doc", null, [chatSchema.node("paragraph")]);
      } catch {
        doc = chatSchema.node("doc", null, [chatSchema.node("paragraph")]);
      }
      return EditorState.create({ doc, plugins });
    }, []);

    return (
      <EditorErrorBoundary>
        <ProseMirror
          defaultState={defaultState}
          nodeViewComponents={nodeViews}
          attributes={{
            spellCheck: "false",
            autoComplete: "off",
            autoCorrect: "off",
            autoCapitalize: "off",
            role: "textbox",
            class: cn(className, "prosemirror-editor"),
          }}
        >
          <ProseMirrorDoc />
          <ViewCapture viewRef={viewRef} />
          {mentionConfig && <MentionSuggestion config={mentionConfig} />}
        </ProseMirror>
      </EditorErrorBoundary>
    );
  },
);
