import { Node as PMNode, type Schema } from "prosemirror-model";
import { EditorState, type Plugin } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { type RefObject, useEffect, useMemo, useState } from "react";

type JSONContent = {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, unknown> }[];
  text?: string;
};

function arePluginsEqual(a: readonly Plugin[], b: readonly Plugin[]) {
  if (a === b) return true;
  if (a.length !== b.length) return false;

  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }

  return true;
}

export function parseEditorDoc(
  schema: Schema,
  content: JSONContent | undefined,
  createEmptyDoc: () => PMNode,
) {
  try {
    if (content?.type === "doc") {
      return PMNode.fromJSON(schema, content);
    }
  } catch {
    // fall through to the empty document
  }

  return createEmptyDoc();
}

export function createEditorState(doc: PMNode, plugins: readonly Plugin[]) {
  return EditorState.create({ doc, plugins });
}

export function reconcileEditorState({
  currentState,
  nextDoc,
  plugins,
  isViewFocused,
}: {
  currentState: EditorState;
  nextDoc: PMNode;
  plugins: readonly Plugin[];
  isViewFocused: boolean;
}) {
  const docChanged = !nextDoc.eq(currentState.doc);
  const pluginsChanged = !arePluginsEqual(currentState.plugins, plugins);

  if (!docChanged && !pluginsChanged) {
    return currentState;
  }

  if (docChanged && !isViewFocused) {
    return createEditorState(nextDoc, plugins);
  }

  if (pluginsChanged) {
    return currentState.reconfigure({ plugins });
  }

  return currentState;
}

export function useManagedEditorState({
  schema,
  initialContent,
  plugins,
  createEmptyDoc,
  viewRef,
}: {
  schema: Schema;
  initialContent?: JSONContent;
  plugins: readonly Plugin[];
  createEmptyDoc: () => PMNode;
  viewRef: RefObject<EditorView | null>;
}) {
  const parsedDoc = useMemo(
    () => parseEditorDoc(schema, initialContent, createEmptyDoc),
    [createEmptyDoc, initialContent, schema],
  );
  const [editorState, setEditorState] = useState(() =>
    createEditorState(parsedDoc, plugins),
  );
  const managedState = useMemo(
    () =>
      reconcileEditorState({
        currentState: editorState,
        nextDoc: parsedDoc,
        plugins,
        isViewFocused: viewRef.current?.hasFocus() ?? false,
      }),
    [editorState, parsedDoc, plugins, viewRef],
  );

  useEffect(() => {
    if (managedState !== editorState) {
      setEditorState(managedState);
    }
  }, [editorState, managedState]);

  return { editorState: managedState, setEditorState };
}
