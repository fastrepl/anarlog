import "prosemirror-gapcursor/style/gapcursor.css";

import {
  ProseMirror,
  ProseMirrorDoc,
  reactKeys,
  useEditorEffect,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { dropCursor } from "prosemirror-dropcursor";
import { gapCursor } from "prosemirror-gapcursor";
import { history } from "prosemirror-history";
import {
  EditorState,
  Selection,
  TextSelection,
  type Transaction,
} from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { forwardRef, useImperativeHandle, useMemo, useRef } from "react";
import { useDebounceCallback } from "usehooks-ts";

import "@hypr/tiptap/styles.css";

import {
  MentionNodeView,
  ResizableImageView,
  TaskItemView,
} from "../node-views";
import {
  type FileHandlerConfig,
  type PlaceholderFunction,
  SearchQuery,
  clearMarksOnEnterPlugin,
  clipPastePlugin,
  fileHandlerPlugin,
  getSearchState,
  hashtagPlugin,
  linkBoundaryGuardPlugin,
  placeholderPlugin,
  searchPlugin,
  searchReplaceAll,
  searchReplaceCurrent,
  setSearchState,
} from "../plugins";
import { useManagedEditorState } from "../state";
import {
  type MentionConfig,
  MentionSuggestion,
  SlashCommandMenu,
  mentionSkipPlugin,
} from "../widgets";
import { buildInputRules, buildKeymap } from "./keymap";
import { schema } from "./schema";

export type { MentionConfig, FileHandlerConfig, PlaceholderFunction };
export { schema };

export interface JSONContent {
  type?: string;
  attrs?: Record<string, any>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, any> }[];
  text?: string;
}

export interface SearchReplaceParams {
  query: string;
  replacement: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  all: boolean;
  matchIndex: number;
}

export interface EditorCommands {
  focus: () => void;
  focusAtStart: () => void;
  focusAtPixelWidth: (pixelWidth: number) => void;
  insertAtStartAndFocus: (content: string) => void;
  setSearch: (query: string, caseSensitive: boolean) => void;
  replace: (params: SearchReplaceParams) => void;
}

export interface NoteEditorRef {
  view: EditorView | null;
  commands: EditorCommands;
}

interface EditorProps {
  handleChange?: (content: JSONContent) => void;
  initialContent?: JSONContent;
  mentionConfig?: MentionConfig;
  placeholderComponent?: PlaceholderFunction;
  fileHandlerConfig?: FileHandlerConfig;
  onNavigateToTitle?: (pixelWidth?: number) => void;
}

const nodeViews = {
  image: ResizableImageView,
  "mention-@": MentionNodeView,
  taskItem: TaskItemView,
};

function createEmptyNoteDoc() {
  return schema.node("doc", null, [schema.node("paragraph")]);
}

function ViewCapture({
  viewRef,
  onViewReady,
}: {
  viewRef: React.RefObject<EditorView | null>;
  onViewReady: (view: EditorView) => void;
}) {
  useEditorEffect((view) => {
    if (view && viewRef.current !== view) {
      viewRef.current = view;
      onViewReady(view);
    }
  });
  return null;
}

const noopCommands: EditorCommands = {
  focus: () => {},
  focusAtStart: () => {},
  focusAtPixelWidth: () => {},
  insertAtStartAndFocus: () => {},
  setSearch: () => {},
  replace: () => {},
};

function EditorCommandsBridge({
  commandsRef,
}: {
  commandsRef: React.RefObject<EditorCommands>;
}) {
  commandsRef.current.focus = useEditorEventCallback((view) => {
    if (!view) return;
    view.focus();
  });

  commandsRef.current.focusAtStart = useEditorEventCallback((view) => {
    if (!view) return;
    view.dispatch(
      view.state.tr.setSelection(Selection.atStart(view.state.doc)),
    );
    view.focus();
  });

  commandsRef.current.focusAtPixelWidth = useEditorEventCallback(
    (view, pixelWidth: number) => {
      if (!view) return;

      const blockStart = Selection.atStart(view.state.doc).from;
      const firstTextNode = view.dom.querySelector(".ProseMirror > *");
      if (firstTextNode) {
        const editorStyle = window.getComputedStyle(firstTextNode);
        const canvas = document.createElement("canvas");
        const ctx = canvas.getContext("2d");
        if (ctx) {
          ctx.font = `${editorStyle.fontWeight} ${editorStyle.fontSize} ${editorStyle.fontFamily}`;
          const firstBlock = view.state.doc.firstChild;
          if (firstBlock && firstBlock.textContent) {
            const text = firstBlock.textContent;
            let charPos = 0;
            for (let i = 0; i <= text.length; i++) {
              const currentWidth = ctx.measureText(text.slice(0, i)).width;
              if (currentWidth >= pixelWidth) {
                charPos = i;
                break;
              }
              charPos = i;
            }
            const targetPos = Math.min(
              blockStart + charPos,
              view.state.doc.content.size - 1,
            );
            view.dispatch(
              view.state.tr.setSelection(
                TextSelection.create(view.state.doc, targetPos),
              ),
            );
            view.focus();
            return;
          }
        }
      }

      view.dispatch(
        view.state.tr.setSelection(Selection.atStart(view.state.doc)),
      );
      view.focus();
    },
  );

  commandsRef.current.insertAtStartAndFocus = useEditorEventCallback(
    (view, content: string) => {
      if (!view || !content) return;
      const pos = Selection.atStart(view.state.doc).from;
      const tr = view.state.tr.insertText(content, pos);
      tr.setSelection(TextSelection.create(tr.doc, pos));
      view.dispatch(tr);
      view.focus();
    },
  );

  commandsRef.current.setSearch = useEditorEventCallback(
    (view, query: string, caseSensitive: boolean) => {
      if (!view) return;
      const q = new SearchQuery({ search: query, caseSensitive });
      const current = getSearchState(view.state);
      if (current && current.query.eq(q)) return;
      view.dispatch(setSearchState(view.state.tr, q));
    },
  );

  commandsRef.current.replace = useEditorEventCallback(
    (view, params: SearchReplaceParams) => {
      if (!view) return;
      const query = new SearchQuery({
        search: params.query,
        replace: params.replacement,
        caseSensitive: params.caseSensitive,
        wholeWord: params.wholeWord,
      });

      view.dispatch(setSearchState(view.state.tr, query));

      if (params.all) {
        searchReplaceAll(view.state, (tr) => view.dispatch(tr));
      } else {
        let result = query.findNext(view.state);
        let idx = 0;
        while (result && idx < params.matchIndex) {
          result = query.findNext(view.state, result.to);
          idx++;
        }
        if (!result) return;
        view.dispatch(
          view.state.tr.setSelection(
            TextSelection.create(view.state.doc, result.from, result.to),
          ),
        );
        searchReplaceCurrent(view.state, (tr) => view.dispatch(tr));
      }
    },
  );

  return null;
}

export const NoteEditor = forwardRef<NoteEditorRef, EditorProps>(
  function NoteEditor(props, ref) {
    const {
      handleChange,
      initialContent,
      mentionConfig,
      placeholderComponent,
      fileHandlerConfig,
      onNavigateToTitle,
    } = props;

    const viewRef = useRef<EditorView | null>(null);
    const commandsRef = useRef<EditorCommands>(noopCommands);
    const handleChangeRef = useRef(handleChange);
    const placeholderRef = useRef(placeholderComponent);
    const fileHandlerConfigRef = useRef(fileHandlerConfig);
    const onNavigateToTitleRef = useRef(onNavigateToTitle);
    handleChangeRef.current = handleChange;
    placeholderRef.current = placeholderComponent;
    fileHandlerConfigRef.current = fileHandlerConfig;
    onNavigateToTitleRef.current = onNavigateToTitle;

    useImperativeHandle(
      ref,
      () => ({
        get view() {
          return viewRef.current;
        },
        get commands() {
          return commandsRef.current;
        },
      }),
      [],
    );

    const onUpdate = useDebounceCallback((state: EditorState) => {
      handleChangeRef.current?.(state.doc.toJSON() as JSONContent);
    }, 500);

    const plugins = useMemo(
      () => [
        reactKeys(),
        buildInputRules(),
        buildKeymap((pixelWidth) => onNavigateToTitleRef.current?.(pixelWidth)),
        history(),
        dropCursor(),
        gapCursor(),
        hashtagPlugin(),
        searchPlugin(),
        placeholderPlugin((props) => placeholderRef.current?.(props) ?? ""),
        clearMarksOnEnterPlugin(),
        clipPastePlugin(),
        linkBoundaryGuardPlugin(),
        ...(mentionConfig ? [mentionSkipPlugin()] : []),
        ...(fileHandlerConfig
          ? [
              fileHandlerPlugin({
                onDrop(files, pos) {
                  return fileHandlerConfigRef.current?.onDrop?.(files, pos);
                },
                onPaste(files) {
                  return fileHandlerConfigRef.current?.onPaste?.(files);
                },
                ...(fileHandlerConfig.onImageUpload
                  ? {
                      async onImageUpload(file: File) {
                        return await fileHandlerConfigRef.current!
                          .onImageUpload!(file);
                      },
                    }
                  : {}),
              }),
            ]
          : []),
      ],
      [
        Boolean(fileHandlerConfig),
        Boolean(fileHandlerConfig?.onImageUpload),
        Boolean(mentionConfig),
      ],
    );

    const { editorState, setEditorState } = useManagedEditorState({
      schema,
      initialContent,
      plugins,
      createEmptyDoc: createEmptyNoteDoc,
      viewRef,
    });

    return (
      <ProseMirror
        state={editorState}
        nodeViews={nodeViews}
        dispatchTransaction={function (this: EditorView, tr: Transaction) {
          const newState = this.state.apply(tr);
          setEditorState(newState);
          if (tr.docChanged) {
            onUpdate(newState);
          }
        }}
        attributes={{
          spellcheck: "false",
          autocomplete: "off",
          autocorrect: "off",
          autocapitalize: "off",
          role: "textbox",
        }}
        className="tiptap"
      >
        <ProseMirrorDoc />
        <ViewCapture
          viewRef={viewRef}
          onViewReady={(view) => {
            onUpdate(view.state);
          }}
        />
        <EditorCommandsBridge commandsRef={commandsRef} />
        <SlashCommandMenu />
        {mentionConfig && <MentionSuggestion config={mentionConfig} />}
      </ProseMirror>
    );
  },
);
