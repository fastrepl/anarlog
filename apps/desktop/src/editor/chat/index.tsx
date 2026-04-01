import "prosemirror-view/style/prosemirror.css";

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
  exitCode,
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
import { EditorState } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { forwardRef, useImperativeHandle, useMemo, useRef } from "react";

import "@hypr/tiptap/styles.css";
import { cn } from "@hypr/utils";

import { isApplePlatform } from "../keyboard";
import { AttachmentChipView, MentionNodeView } from "../node-views";
import {
  createDropPasteFileHandlerPlugin,
  type PlaceholderFunction,
  placeholderPlugin,
} from "../plugins";
import { createEditorState, useManagedEditorState } from "../state";
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
  focus(): void;
  getJSON(): JSONContent | undefined;
  clearContent(): void;
}

interface ChatEditorProps {
  className?: string;
  initialContent?: JSONContent;
  mentionConfig?: MentionConfig;
  placeholder?: PlaceholderFunction;
  onUpdate?: (json: JSONContent) => void;
  onSubmit?: () => void;
}

const nodeViews = {
  "mention-@": MentionNodeView,
  attachment: AttachmentChipView,
};

function createEmptyChatDoc() {
  return chatSchema.node("doc", null, [chatSchema.node("paragraph")]);
}

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

const mac = isApplePlatform();

function fileHandlerPlugin() {
  return createDropPasteFileHandlerPlugin({
    key: "chatFileHandler",
    shouldHandleFile: () => true,
    handleFiles: insertFiles,
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
      onUpdate,
      onSubmit,
    } = props;

    const viewRef = useRef<EditorView | null>(null);
    const onSubmitRef = useRef(onSubmit);
    onSubmitRef.current = onSubmit;
    const onUpdateRef = useRef(onUpdate);
    const placeholderRef = useRef(placeholder);
    const mentionConfigRef = useRef(mentionConfig);
    onUpdateRef.current = onUpdate;
    placeholderRef.current = placeholder;
    mentionConfigRef.current = mentionConfig;

    useImperativeHandle(
      ref,
      () => ({
        focus() {
          viewRef.current?.focus();
        },
        getJSON() {
          return viewRef.current?.state.doc.toJSON() as JSONContent | undefined;
        },
        clearContent() {
          setEditorState((currentState) => {
            const nextState = createEditorState(
              createEmptyChatDoc(),
              currentState.plugins,
            );
            onUpdateRef.current?.(nextState.doc.toJSON() as JSONContent);
            return nextState;
          });
        },
      }),
      [],
    );

    const plugins = useMemo(
      () => [
        reactKeys(),
        keymap({
          "Mod-z": undo,
          "Mod-Shift-z": redo,
          ...(!mac ? { "Mod-y": redo } : {}),
          "Mod-Enter": (state: EditorState) => {
            const currentMentionConfig = mentionConfigRef.current;
            if (
              currentMentionConfig &&
              findMention(state, currentMentionConfig.trigger)
            ) {
              return false;
            }
            onSubmitRef.current?.();
            return true;
          },
          "Shift-Enter": chainCommands(exitCode, (state, dispatch) => {
            if (dispatch) {
              dispatch(
                state.tr
                  .replaceSelectionWith(chatSchema.nodes.hardBreak.create())
                  .scrollIntoView(),
              );
            }
            return true;
          }),
          Enter: chainCommands(createParagraphNear, liftEmptyBlock, splitBlock),
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
        }),
        history(),
        placeholderPlugin((props) => placeholderRef.current?.(props) ?? ""),
        ...(mentionConfig ? [mentionSkipPlugin()] : []),
        fileHandlerPlugin(),
      ],
      [Boolean(mentionConfig)],
    );

    const { editorState, setEditorState } = useManagedEditorState({
      schema: chatSchema,
      initialContent,
      plugins,
      createEmptyDoc: createEmptyChatDoc,
      viewRef,
    });

    return (
      <ProseMirror
        state={editorState}
        nodeViews={nodeViews}
        dispatchTransaction={function (this: EditorView, tr) {
          const newState = this.state.apply(tr);
          setEditorState(newState);
          if (tr.docChanged) {
            onUpdateRef.current?.(newState.doc.toJSON() as JSONContent);
          }
        }}
        attributes={{
          spellcheck: "false",
          autocomplete: "off",
          autocorrect: "off",
          autocapitalize: "off",
          role: "textbox",
        }}
        className={cn(className, "tiptap")}
      >
        <ProseMirrorDoc />
        <ViewCapture viewRef={viewRef} />
        {mentionConfig && <MentionSuggestion config={mentionConfig} />}
      </ProseMirror>
    );
  },
);
