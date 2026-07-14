import "prosemirror-view/style/prosemirror.css";
import "../styles/prosemirror.css";

import {
  ProseMirror,
  ProseMirrorDoc,
  reactKeys,
  useEditorEffect,
} from "@handlewithcare/react-prosemirror";
import { baseKeymap } from "prosemirror-commands";
import { history, redo, undo } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { type Node as PMNode, type NodeSpec, Schema } from "prosemirror-model";
import { EditorState, Plugin, PluginKey } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import {
  type Ref,
  type RefObject,
  useImperativeHandle,
  useMemo,
  useRef,
} from "react";

import { cn } from "@hypr/utils";

import { EditorErrorBoundary } from "../editor-error-boundary";
import { docChangeListenerPlugin, placeholderPlugin } from "../plugins";

export type PromptTokenName = "template";

const TOKEN_PATTERN = /\{\{\s*(template)\s*\}\}/g;

const promptTokenNodeSpec: NodeSpec = {
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  attrs: { name: { default: "template" } },
  parseDOM: [
    {
      tag: "span[data-prompt-token]",
      getAttrs(dom) {
        const name = (dom as HTMLElement).getAttribute("data-prompt-token");
        return name === "template" ? { name } : false;
      },
    },
  ],
  toDOM(node) {
    return [
      "span",
      {
        class: "prompt-token",
        contenteditable: "false",
        "data-prompt-token": node.attrs.name,
      },
      "Template",
    ];
  },
};

export const promptSchema = new Schema({
  nodes: {
    doc: { content: "paragraph+" },
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{ tag: "p" }],
      toDOM() {
        return ["p", 0];
      },
    },
    text: { group: "inline" },
    hardBreak: {
      inline: true,
      group: "inline",
      selectable: false,
      parseDOM: [{ tag: "br" }],
      toDOM() {
        return ["br"];
      },
    },
    promptToken: promptTokenNodeSpec,
  },
});

export function promptTextToDoc(value: string): PMNode {
  const paragraphs = value
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((line) => {
      const content: PMNode[] = [];
      let cursor = 0;

      for (const match of line.matchAll(TOKEN_PATTERN)) {
        const index = match.index;
        if (index > cursor) {
          content.push(promptSchema.text(line.slice(cursor, index)));
        }
        content.push(
          promptSchema.nodes.promptToken.create({ name: "template" }),
        );
        cursor = index + match[0].length;
      }

      if (cursor < line.length) {
        content.push(promptSchema.text(line.slice(cursor)));
      }

      return promptSchema.nodes.paragraph.create(null, content);
    });

  return promptSchema.nodes.doc.create(null, paragraphs);
}

export function promptDocToText(doc: PMNode): string {
  const paragraphs: string[] = [];

  doc.forEach((paragraph) => {
    let value = "";
    paragraph.forEach((node) => {
      if (node.isText) {
        value += node.text ?? "";
      } else if (node.type.name === "promptToken") {
        value += `{{ ${String(node.attrs.name)} }}`;
      } else if (node.type.name === "hardBreak") {
        value += "\n";
      }
    });
    paragraphs.push(value);
  });

  return paragraphs.join("\n");
}

export interface PromptEditorHandle {
  focus(): void;
  insertToken(name: PromptTokenName): void;
  setValue(value: string): void;
}

export function PromptEditor({
  ref,
  ariaLabel,
  className,
  initialValue,
  maxLength,
  onBlur,
  onChange,
  placeholder,
}: {
  ref?: Ref<PromptEditorHandle>;
  ariaLabel: string;
  className?: string;
  initialValue: string;
  maxLength?: number;
  onBlur?: () => void;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  const viewRef = useRef<EditorView | null>(null);
  const onBlurRef = useRef(onBlur);
  onBlurRef.current = onBlur;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useImperativeHandle(
    ref,
    () => ({
      focus() {
        viewRef.current?.focus();
      },
      insertToken(name) {
        const view = viewRef.current;
        if (!view) return;

        const token = promptSchema.nodes.promptToken.create({ name });
        view.dispatch(
          view.state.tr.replaceSelectionWith(token).scrollIntoView(),
        );
        view.focus();
      },
      setValue(value) {
        const view = viewRef.current;
        if (!view) return;

        const doc = promptTextToDoc(value);
        view.dispatch(
          view.state.tr.replaceWith(
            0,
            view.state.doc.content.size,
            doc.content,
          ),
        );
      },
    }),
    [],
  );

  const plugins = useMemo(
    () => [
      reactKeys(),
      docChangeListenerPlugin((doc) => {
        onChangeRef.current(promptDocToText(doc));
      }),
      keymap({ "Mod-z": undo, "Mod-Shift-z": redo, "Mod-y": redo }),
      keymap(baseKeymap),
      history(),
      placeholderPlugin(() => placeholder ?? ""),
      new Plugin({
        key: new PluginKey("promptEditorEvents"),
        filterTransaction(transaction) {
          return (
            !transaction.docChanged ||
            !maxLength ||
            promptDocToText(transaction.doc).length <= maxLength
          );
        },
        props: {
          handleDOMEvents: {
            blur() {
              onBlurRef.current?.();
              return false;
            },
          },
        },
      }),
    ],
    [maxLength, placeholder],
  );

  const defaultState = useMemo(
    () =>
      EditorState.create({
        doc: promptTextToDoc(initialValue),
        plugins,
      }),
    [initialValue, plugins],
  );
  const editorKey = `${maxLength ?? ""}:${placeholder ?? ""}`;

  return (
    <EditorErrorBoundary>
      <ProseMirror
        key={editorKey}
        defaultState={defaultState}
        attributes={{
          "aria-label": ariaLabel,
          autoCapitalize: "sentences",
          autoComplete: "off",
          autoCorrect: "on",
          class: cn(["prosemirror-editor prompt-editor", className]),
          role: "textbox",
          spellCheck: "true",
        }}
      >
        <ProseMirrorDoc />
        <ViewCapture viewRef={viewRef} />
      </ProseMirror>
    </EditorErrorBoundary>
  );
}

function ViewCapture({ viewRef }: { viewRef: RefObject<EditorView | null> }) {
  useEditorEffect((view) => {
    if (view && viewRef.current !== view) viewRef.current = view;
  });
  return null;
}
