import "prosemirror-view/style/prosemirror.css";
import "prosemirror-gapcursor/style/gapcursor.css";
import "../styles/prosemirror.css";

import {
  ProseMirror,
  ProseMirrorDoc,
  reactKeys,
  useEditorEffect,
} from "@handlewithcare/react-prosemirror";
import { dropCursor } from "prosemirror-dropcursor";
import { gapCursor } from "prosemirror-gapcursor";
import { history } from "prosemirror-history";
import { Node as PMNode } from "prosemirror-model";
import {
  EditorState,
  Plugin,
  PluginKey,
  Selection,
  TextSelection,
} from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import {
  type ComponentProps,
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
} from "react";
import { useDebounceCallback } from "usehooks-ts";

import { cn } from "@hypr/utils";

import { EditorErrorBoundary } from "../editor-error-boundary";
import {
  FileAttachmentView,
  getNodeViewFallbackTag,
  MentionNodeView,
  ResizableImageView,
  TaskItemView,
  withNodeViewErrorBoundary,
} from "../node-views";
import {
  appLinkPastePlugin,
  autolinkPlugin,
  type FileHandlerConfig,
  type PlaceholderFunction,
  SearchQuery,
  clearMarksOnEnterPlugin,
  clipboardPlugin,
  clipPastePlugin,
  docChangeListenerPlugin,
  ensureImageTrailingParagraphs,
  fileHandlerPlugin,
  getSearchState,
  hashtagPlugin,
  imageTrailingParagraphPlugin,
  type LinkOpenHandler,
  linkBoundaryGuardPlugin,
  linkOpenPlugin,
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
  useLinkedItemOpenBehavior,
} from "./linked-item-open-behavior";
import { schema } from "./schema";
import { normalizeTitleHeadingDoc, titleHeadingPlugin } from "./title-layout";
import { trailingEmptyLineClickPlugin } from "./trailing-empty-line-click";

export type { MentionConfig, FileHandlerConfig, PlaceholderFunction };
export { schema };
export { useLinkedItemOpenBehavior };

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

export type SessionMentionDropData = {
  id: string;
  label: string;
};

export type SessionMentionDropConfig = {
  has?: (
    dataTransfer: Pick<DataTransfer, "types"> | null | undefined,
  ) => boolean;
  read: (
    dataTransfer: Pick<DataTransfer, "getData" | "types"> | null | undefined,
  ) => SessionMentionDropData | null;
};

type NodeViewComponents = NonNullable<
  ComponentProps<typeof ProseMirror>["nodeViewComponents"]
>;

export interface NoteEditorProps {
  className?: string;
  handleChange?: (content: JSONContent) => void;
  initialContent?: JSONContent;
  mentionConfig?: MentionConfig;
  placeholderComponent?: PlaceholderFunction;
  fileHandlerConfig?: FileHandlerConfig;
  onNavigateToTitle?: (pixelWidth?: number) => void;
  onLinkOpen?: LinkOpenHandler;
  linkedItemOpenBehavior?: LinkedItemOpenBehavior;
  taskSource?: TaskSource;
  extraNodeViews?: NodeViewComponents;
  sessionMentionDropConfig?: SessionMentionDropConfig;
  showFormatToolbar?: boolean;
  onViewReady?: (view: EditorView) => void;
  onViewDisposed?: (view: EditorView) => void;
  syncContentWhenFocused?: boolean;
  enforceTitleHeading?: boolean;
}

const baseNodeViews = {
  fileAttachment: withNodeViewErrorBoundary<HTMLDivElement>(
    FileAttachmentView,
    { name: "fileAttachment" },
  ),
  image: withNodeViewErrorBoundary<HTMLDivElement>(ResizableImageView, {
    name: "image",
  }),
  "mention-@": withNodeViewErrorBoundary<HTMLElement>(MentionNodeView, {
    name: "mention-@",
  }),
  taskItem: withNodeViewErrorBoundary<HTMLLIElement>(TaskItemView, {
    name: "taskItem",
  }),
};

function isSameContent(
  left: JSONContent | undefined,
  right: JSONContent | undefined,
) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function wrapNodeViewComponents(nodeViews?: NodeViewComponents) {
  if (!nodeViews) {
    return {};
  }

  return Object.fromEntries(
    Object.entries(nodeViews).map(([name, Component]) => [
      name,
      withNodeViewErrorBoundary(Component, {
        fallbackTag: getNodeViewFallbackTag(name),
        name,
      }),
    ]),
  ) as NodeViewComponents;
}

function createSessionMentionDropPlugin(config: SessionMentionDropConfig) {
  const hasSession = (dataTransfer: DataTransfer | null) =>
    config.has ? config.has(dataTransfer) : Boolean(config.read(dataTransfer));
  const readSession = (dataTransfer: DataTransfer | null) =>
    config.read(dataTransfer);

  return new Plugin({
    key: new PluginKey("sessionMentionDrop"),
    props: {
      handleDOMEvents: {
        dragover(_view, event) {
          const dataTransfer = (event as DragEvent).dataTransfer;
          if (!hasSession(dataTransfer)) {
            return false;
          }

          event.preventDefault();
          if (dataTransfer) {
            dataTransfer.dropEffect = "copy";
          }
          return true;
        },
      },
      handleDrop(view, event) {
        const session = readSession(event.dataTransfer);
        const mentionType = view.state.schema.nodes["mention-@"];
        if (!session || !mentionType) {
          return false;
        }

        event.preventDefault();
        event.stopPropagation();

        const mentionNode = mentionType.create({
          id: session.id,
          type: "session",
          label: session.label,
        });
        const space = view.state.schema.text(" ");
        const pos =
          view.posAtCoords({
            left: event.clientX,
            top: event.clientY,
          })?.pos ?? view.state.selection.from;

        try {
          const tr = view.state.tr.insert(pos, [mentionNode, space]);
          tr.setSelection(
            TextSelection.create(
              tr.doc,
              pos + mentionNode.nodeSize + space.nodeSize,
            ),
          );
          view.dispatch(tr.scrollIntoView());
        } catch {
          const tr = view.state.tr
            .replaceSelectionWith(mentionNode)
            .insertText(" ");
          view.dispatch(tr.scrollIntoView());
        }

        view.focus();
        return true;
      },
    },
  });
}

function ViewCapture({
  viewRef,
  onViewReady,
  onViewDisposed,
}: {
  viewRef: React.RefObject<EditorView | null>;
  onViewReady: (view: EditorView) => void;
  onViewDisposed?: (view: EditorView) => void;
}) {
  const callbacksRef = useRef({ onViewReady, onViewDisposed });
  callbacksRef.current = { onViewReady, onViewDisposed };

  useEditorEffect(
    (view) => {
      if (view && viewRef.current !== view) {
        viewRef.current = view;
        callbacksRef.current.onViewReady(view);
      }

      return () => {
        if (viewRef.current === view) {
          viewRef.current = null;
          callbacksRef.current.onViewDisposed?.(view);
        }
      };
    },
    [viewRef],
  );
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
  useEditorEffect(
    (view) => {
      if (!view) {
        commandsRef.current = noopCommands;
        return;
      }

      commandsRef.current = {
        focus: () => {
          view.focus();
        },
        focusAtStart: () => {
          view.dispatch(
            view.state.tr.setSelection(Selection.atStart(view.state.doc)),
          );
          view.focus();
        },
        focusAtPixelWidth: (pixelWidth: number) => {
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
        insertAtStartAndFocus: (content: string) => {
          if (!content) return;
          const pos = Selection.atStart(view.state.doc).from;
          const tr = view.state.tr.insertText(content, pos);
          tr.setSelection(TextSelection.create(tr.doc, pos));
          view.dispatch(tr);
          view.focus();
        },
        replaceContent: (content: JSONContent) => {
          if (content.type !== "doc") return;

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
        setSearch: (query: string, caseSensitive: boolean) => {
          const q = new SearchQuery({ search: query, caseSensitive });
          const current = getSearchState(view.state);
          if (current && current.query.eq(q)) return;
          view.dispatch(setSearchState(view.state.tr, q));
        },
        replace: (params: SearchReplaceParams) => {
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
      };

      return () => {
        commandsRef.current = noopCommands;
      };
    },
    [commandsRef],
  );

  return null;
}

export const NoteEditor = forwardRef<NoteEditorRef, NoteEditorProps>(
  function NoteEditor(props, ref) {
    const {
      handleChange,
      className,
      initialContent,
      mentionConfig,
      placeholderComponent,
      fileHandlerConfig,
      onNavigateToTitle,
      onLinkOpen,
      linkedItemOpenBehavior = "current",
      taskSource,
      extraNodeViews,
      sessionMentionDropConfig,
      showFormatToolbar = true,
      onViewReady: onViewReadyProp,
      onViewDisposed,
      syncContentWhenFocused = false,
      enforceTitleHeading = true,
    } = props;

    const taskStorage = useTaskStorageOptional();
    const normalizedInitialContent = useMemo(
      () => normalizeTaskContent(initialContent),
      [initialContent],
    );
    const reconciledInitialContent = useMemo(() => {
      if (!normalizedInitialContent) {
        return normalizedInitialContent;
      }

      const hydrated =
        taskSource && taskStorage
          ? (() => {
              const sourceTasks = taskStorage.getTasksForSource(taskSource);
              if (sourceTasks.length === 0) return normalizedInitialContent;
              return hydrateTaskContent({
                content: normalizedInitialContent,
                sourceTasks,
                getTask: taskStorage.getTask,
              });
            })()
          : normalizedInitialContent;

      return ensureImageTrailingParagraphs(hydrated);
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

    const flushChange = useCallback(
      (content: JSONContent) => {
        syncTasks(content);
        if (!handleChange) {
          return;
        }

        handleChange(content);
      },
      [handleChange, syncTasks],
    );

    const onUpdate = useDebounceCallback(flushChange, 500);
    const onUpdateRef = useRef(onUpdate);
    onUpdateRef.current = onUpdate;

    const plugins = useMemo(
      () => [
        reactKeys(),
        docChangeListenerPlugin((view) =>
          onUpdateRef.current(view.state.doc.toJSON() as JSONContent),
        ),
        buildInputRules(),
        ...(enforceTitleHeading ? [titleHeadingPlugin()] : []),
        taskIdentityPlugin(),
        buildKeymap(onNavigateToTitle),
        trailingEmptyLineClickPlugin(),
        history(),
        dropCursor(),
        gapCursor(),
        clipboardPlugin(),
        hashtagPlugin(),
        imageTrailingParagraphPlugin(),
        searchPlugin(),
        placeholderPlugin(placeholderComponent),
        clearMarksOnEnterPlugin(),
        clipPastePlugin(),
        appLinkPastePlugin(),
        autolinkPlugin(),
        linkBoundaryGuardPlugin(),
        ...(onLinkOpen ? [linkOpenPlugin(onLinkOpen)] : []),
        ...(mentionConfig ? [mentionSkipPlugin()] : []),
        ...(sessionMentionDropConfig
          ? [createSessionMentionDropPlugin(sessionMentionDropConfig)]
          : []),
        ...(fileHandlerConfig ? [fileHandlerPlugin(fileHandlerConfig)] : []),
      ],
      [
        placeholderComponent,
        fileHandlerConfig,
        mentionConfig,
        sessionMentionDropConfig,
        onNavigateToTitle,
        onLinkOpen,
        enforceTitleHeading,
      ],
    );
    const nodeViews = useMemo(
      () => ({ ...baseNodeViews, ...wrapNodeViewComponents(extraNodeViews) }),
      [extraNodeViews],
    );

    const defaultState = useMemo(() => {
      let doc: PMNode;
      try {
        doc =
          reconciledInitialContent && reconciledInitialContent.type === "doc"
            ? PMNode.fromJSON(schema, reconciledInitialContent)
            : schema.node("doc", null, [schema.node("paragraph")]);
        if (enforceTitleHeading) {
          doc = normalizeTitleHeadingDoc(doc);
        }
      } catch {
        doc = schema.node("doc", null, [
          enforceTitleHeading
            ? schema.node("heading", { level: 1 })
            : schema.node("paragraph"),
        ]);
      }
      return EditorState.create({ doc, plugins });
    }, [reconciledInitialContent, plugins, enforceTitleHeading]);

    useEffect(() => {
      const view = viewRef.current;
      if (!view) return;
      if (previousContentRef.current === reconciledInitialContent) return;

      if (
        !reconciledInitialContent ||
        reconciledInitialContent.type !== "doc"
      ) {
        return;
      }

      const currentContent = view.state.doc.toJSON() as JSONContent;
      if (isSameContent(currentContent, reconciledInitialContent)) {
        previousContentRef.current = reconciledInitialContent;
        return;
      }

      if (view.hasFocus() && !syncContentWhenFocused) {
        previousContentRef.current = reconciledInitialContent;
        return;
      }

      try {
        let doc = PMNode.fromJSON(schema, reconciledInitialContent);
        if (enforceTitleHeading) {
          doc = normalizeTitleHeadingDoc(doc);
        }
        const state = EditorState.create({
          doc,
          plugins: view.state.plugins,
        });
        onUpdate.cancel();
        view.updateState(state);
        previousContentRef.current = reconciledInitialContent;
      } catch {
        // invalid content
      }
    }, [
      reconciledInitialContent,
      syncContentWhenFocused,
      enforceTitleHeading,
      onUpdate,
    ]);

    const onViewReady = useCallback(
      (view: EditorView) => {
        onViewReadyProp?.(view);
        syncTasks(view.state.doc.toJSON() as JSONContent);
      },
      [onViewReadyProp, syncTasks],
    );

    return (
      <TaskSourceProvider source={taskSource ?? null}>
        <LinkedItemOpenBehaviorContext.Provider value={linkedItemOpenBehavior}>
          <EditorErrorBoundary
            resetKey={
              taskSource ? `${taskSource.type}:${taskSource.id}` : "note"
            }
          >
            <ProseMirror
              defaultState={defaultState}
              nodeViewComponents={nodeViews}
              attributes={{
                spellCheck: "false",
                autoComplete: "off",
                autoCorrect: "off",
                autoCapitalize: "off",
                role: "textbox",
                class: cn([
                  "prosemirror-editor",
                  enforceTitleHeading && "note-title-editor",
                  className,
                ]),
              }}
            >
              <ProseMirrorDoc />
              <ViewCapture
                viewRef={viewRef}
                onViewReady={onViewReady}
                onViewDisposed={onViewDisposed}
              />
              <EditorCommandsBridge commandsRef={commandsRef} />
              {showFormatToolbar && <FormatToolbar />}
              <SlashCommandMenu />
              {mentionConfig && <MentionSuggestion config={mentionConfig} />}
            </ProseMirror>
          </EditorErrorBoundary>
        </LinkedItemOpenBehaviorContext.Provider>
      </TaskSourceProvider>
    );
  },
);
