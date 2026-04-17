import "prosemirror-view/style/prosemirror.css";
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
import { Node as PMNode } from "prosemirror-model";
import {
  EditorState,
  Selection,
  TextSelection,
  type Transaction,
} from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
} from "react";
import { useDebounceCallback } from "usehooks-ts";

import "@hypr/tiptap/styles.css";

import {
  AppLinkView,
  FileAttachmentView,
  MentionNodeView,
  ResizableImageView,
  SessionNodeView,
  TaskItemView,
} from "../node-views";
import {
  appLinkPastePlugin,
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
  taskIdentityPlugin,
} from "../plugins";
import { TaskSourceProvider } from "../task-source";
import { useTaskStorageOptional } from "../task-storage";
import {
  extractTasksFromContent,
  hydrateTaskContent,
  normalizeTaskContent,
  type TaskSource,
} from "../tasks";
import {
  FormatToolbar,
  type MentionConfig,
  MentionSuggestion,
  SlashCommandMenu,
  mentionSkipPlugin,
} from "../widgets";
import { buildInputRules, buildKeymap } from "./keymap";
import {
  LinkedItemOpenBehaviorContext,
  type LinkedItemOpenBehavior,
} from "./linked-item-open-behavior";
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
  replaceContent: (content: JSONContent) => void;
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
  linkedItemOpenBehavior?: LinkedItemOpenBehavior;
  taskSource?: TaskSource;
}

const nodeViews = {
  appLink: AppLinkView,
  fileAttachment: FileAttachmentView,
  image: ResizableImageView,
  "mention-@": MentionNodeView,
  session: SessionNodeView,
  taskItem: TaskItemView,
};

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
  replaceContent: () => {},
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

  commandsRef.current.replaceContent = useEditorEventCallback(
    (view, content: JSONContent) => {
      if (!view || content.type !== "doc") return;

      try {
        const nextDoc = PMNode.fromJSON(schema, content);
        if (nextDoc.eq(view.state.doc)) {
          return;
        }

        view.dispatch(
          view.state.tr.replaceWith(
            0,
            view.state.doc.content.size,
            nextDoc.content,
          ),
        );
      } catch {
        // invalid content
      }
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
      linkedItemOpenBehavior = "current",
      taskSource,
    } = props;

    const taskStorage = useTaskStorageOptional();
    const normalizedInitialContent = useMemo(
      () => normalizeTaskContent(initialContent),
      [initialContent],
    );
    const reconciledInitialContent = useMemo(() => {
      if (!normalizedInitialContent || !taskSource || !taskStorage) {
        return normalizedInitialContent;
      }

      const sourceTasks = taskStorage.getTasksForSource(taskSource);
      if (sourceTasks.length === 0) {
        return normalizedInitialContent;
      }

      return hydrateTaskContent({
        content: normalizedInitialContent,
        sourceTasks,
        getTask: taskStorage.getTask,
      });
    }, [normalizedInitialContent, taskSource, taskStorage]);
    const previousContentRef = useRef<JSONContent | undefined>(
      reconciledInitialContent,
    );
    const viewRef = useRef<EditorView | null>(null);
    const commandsRef = useRef<EditorCommands>(noopCommands);

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

    const syncTasks = useCallback(
      (content: JSONContent) => {
        if (!taskSource || !taskStorage) {
          return;
        }

        const previousTasks = new Map(
          taskStorage
            .getTasksForSource(taskSource)
            .map((task) => [task.taskId, task]),
        );
        taskStorage.upsertTasksForSource(
          taskSource,
          extractTasksFromContent(content, taskSource, previousTasks),
        );
      },
      [taskSource, taskStorage],
    );

    const onUpdate = useDebounceCallback((view: EditorView) => {
      const content = view.state.doc.toJSON() as JSONContent;
      syncTasks(content);
      if (!handleChange) {
        return;
      }

      handleChange(content);
    }, 500);

    const plugins = useMemo(
      () => [
        reactKeys(),
        buildInputRules(),
        taskIdentityPlugin(),
        buildKeymap(onNavigateToTitle),
        history(),
        dropCursor(),
        gapCursor(),
        hashtagPlugin(),
        searchPlugin(),
        placeholderPlugin(placeholderComponent),
        clearMarksOnEnterPlugin(),
        clipPastePlugin(),
        appLinkPastePlugin(),
        linkBoundaryGuardPlugin(),
        ...(mentionConfig ? [mentionSkipPlugin()] : []),
        ...(fileHandlerConfig ? [fileHandlerPlugin(fileHandlerConfig)] : []),
      ],
      [
        placeholderComponent,
        fileHandlerConfig,
        mentionConfig,
        onNavigateToTitle,
      ],
    );

    const defaultState = useMemo(() => {
      let doc: PMNode;
      try {
        doc =
          reconciledInitialContent && reconciledInitialContent.type === "doc"
            ? PMNode.fromJSON(schema, reconciledInitialContent)
            : schema.node("doc", null, [schema.node("paragraph")]);
      } catch {
        doc = schema.node("doc", null, [schema.node("paragraph")]);
      }
      return EditorState.create({ doc, plugins });
    }, [reconciledInitialContent, plugins]);

    useEffect(() => {
      const view = viewRef.current;
      if (!view) return;
      if (previousContentRef.current === reconciledInitialContent) return;
      previousContentRef.current = reconciledInitialContent;

      if (
        !reconciledInitialContent ||
        reconciledInitialContent.type !== "doc"
      ) {
        return;
      }

      if (!view.hasFocus()) {
        try {
          const doc = PMNode.fromJSON(schema, reconciledInitialContent);
          const state = EditorState.create({
            doc,
            plugins: view.state.plugins,
          });
          view.updateState(state);
        } catch {
          // invalid content
        }
      }
    }, [reconciledInitialContent]);

    const onViewReady = useCallback(
      (view: EditorView) => {
        onUpdate(view);
      },
      [onUpdate],
    );

    return (
      <TaskSourceProvider source={taskSource ?? null}>
        <LinkedItemOpenBehaviorContext.Provider value={linkedItemOpenBehavior}>
          <ProseMirror
            defaultState={defaultState}
            nodeViewComponents={nodeViews}
            dispatchTransaction={function (this: EditorView, tr: Transaction) {
              const { state: newState, transactions } =
                this.state.applyTransaction(tr);
              this.updateState(newState);
              if (transactions.some((transaction) => transaction.docChanged)) {
                onUpdate(this);
              }
            }}
            attributes={{
              spellcheck: "false",
              autocomplete: "off",
              autocorrect: "off",
              autocapitalize: "off",
              role: "textbox",
              class: "tiptap",
            }}
          >
            <ProseMirrorDoc />
            <ViewCapture viewRef={viewRef} onViewReady={onViewReady} />
            <EditorCommandsBridge commandsRef={commandsRef} />
            <FormatToolbar />
            <SlashCommandMenu />
            {mentionConfig && <MentionSuggestion config={mentionConfig} />}
          </ProseMirror>
        </LinkedItemOpenBehaviorContext.Provider>
      </TaskSourceProvider>
    );
  },
);
