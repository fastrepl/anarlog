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
import { EditorState, type Transaction } from "prosemirror-state";
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

import { ResizableImageView } from "./image-view";
import { buildInputRules, buildKeymap } from "./keymap";
import {
  type MentionConfig,
  MentionNodeView,
  MentionSuggestion,
  mentionSkipPlugin,
  mentionSuggestionPlugin,
} from "./mention";
import {
  type FileHandlerConfig,
  type PlaceholderFunction,
  type SearchAndReplaceStorage,
  clearMarksOnEnterPlugin,
  clipPastePlugin,
  createSearchStorage,
  fileHandlerPlugin,
  hashtagPlugin,
  linkBoundaryGuardPlugin,
  placeholderPlugin,
  searchAndReplacePlugin,
} from "./plugins";
import { schema } from "./schema";

export type { MentionConfig, FileHandlerConfig, PlaceholderFunction };
export { schema };
export type { SearchAndReplaceStorage };

export interface JSONContent {
  type?: string;
  attrs?: Record<string, any>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, any> }[];
  text?: string;
}

export interface NoteEditorRef {
  view: EditorView | null;
  searchStorage: SearchAndReplaceStorage;
}

interface EditorProps {
  handleChange?: (content: JSONContent) => void;
  initialContent?: JSONContent;
  mentionConfig?: MentionConfig;
  placeholderComponent?: PlaceholderFunction;
  fileHandlerConfig?: FileHandlerConfig;
  onNavigateToTitle?: () => void;
}

const nodeViews = {
  image: ResizableImageView,
  "mention-@": MentionNodeView,
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

const NoteEditor = forwardRef<NoteEditorRef, EditorProps>((props, ref) => {
  const {
    handleChange,
    initialContent,
    mentionConfig,
    placeholderComponent,
    fileHandlerConfig,
    onNavigateToTitle,
  } = props;

  const previousContentRef = useRef<JSONContent | undefined>(initialContent);
  const searchStorage = useMemo(() => createSearchStorage(), []);
  const viewRef = useRef<EditorView | null>(null);

  useImperativeHandle(ref, () => ({ view: viewRef.current, searchStorage }), [
    searchStorage,
  ]);

  const onUpdate = useDebounceCallback((view: EditorView) => {
    if (!handleChange) return;
    handleChange(view.state.doc.toJSON() as JSONContent);
  }, 500);

  const plugins = useMemo(
    () => [
      reactKeys(),
      buildInputRules(),
      buildKeymap(onNavigateToTitle),
      history(),
      dropCursor(),
      gapCursor(),
      hashtagPlugin(),
      searchAndReplacePlugin(searchStorage),
      placeholderPlugin(placeholderComponent),
      clearMarksOnEnterPlugin(),
      clipPastePlugin(),
      linkBoundaryGuardPlugin(),
      ...(mentionConfig
        ? [mentionSuggestionPlugin(mentionConfig.trigger), mentionSkipPlugin()]
        : []),
      ...(fileHandlerConfig ? [fileHandlerPlugin(fileHandlerConfig)] : []),
    ],
    [
      searchStorage,
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
        initialContent && initialContent.type === "doc"
          ? PMNode.fromJSON(schema, initialContent)
          : schema.node("doc", null, [schema.node("paragraph")]);
    } catch {
      doc = schema.node("doc", null, [schema.node("paragraph")]);
    }
    return EditorState.create({ doc, plugins });
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    if (previousContentRef.current === initialContent) return;
    previousContentRef.current = initialContent;

    if (!initialContent || initialContent.type !== "doc") return;

    if (!view.hasFocus()) {
      try {
        const doc = PMNode.fromJSON(schema, initialContent);
        const state = EditorState.create({
          doc,
          plugins: view.state.plugins,
        });
        view.updateState(state);
      } catch {
        // invalid content
      }
    }
  }, [initialContent]);

  const onViewReady = useCallback(
    (view: EditorView) => {
      onUpdate(view);
    },
    [onUpdate],
  );

  return (
    <ProseMirror
      defaultState={defaultState}
      nodeViews={nodeViews}
      dispatchTransaction={function (this: EditorView, tr: Transaction) {
        const newState = this.state.apply(tr);
        this.updateState(newState);
        if (tr.docChanged) {
          onUpdate(this);
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
      <ViewCapture viewRef={viewRef} onViewReady={onViewReady} />
      {mentionConfig && <MentionSuggestion config={mentionConfig} />}
    </ProseMirror>
  );
});

NoteEditor.displayName = "NoteEditor";

export default NoteEditor;
