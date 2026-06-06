import "./theme.css";

import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

const templateTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontFamily: "var(--font-mono, 'Menlo', 'Monaco', 'Courier New', monospace)",
    fontSize: "13px",
    lineHeight: "1.6",
  },
  ".cm-content": {
    padding: "8px 0",
  },
  ".cm-line": {
    padding: "0 12px",
  },
  ".cm-scroller": {
    overflow: "auto",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-completionIcon": {
    paddingRight: "6px",
  },
  ".cm-tooltip.cm-completionInfo": {
    padding: "4px 8px",
  },
  ".cm-placeholder": {
    color: "var(--cm-placeholder)",
    fontStyle: "italic",
  },
});

const templateHighlightStyle = HighlightStyle.define([
  {
    tag: tags.brace,
    color: "var(--cm-syntax-brace)",
    fontWeight: "600",
  },
  {
    tag: tags.variableName,
    color: "var(--cm-syntax-variable)",
    fontWeight: "500",
  },
  {
    tag: tags.keyword,
    color: "var(--cm-syntax-keyword)",
    fontWeight: "500",
  },
  { tag: tags.string, color: "var(--cm-syntax-string)" },
  { tag: tags.operator, color: "var(--cm-syntax-operator)" },
  {
    tag: tags.propertyName,
    color: "var(--cm-syntax-property)",
  },
  {
    tag: tags.comment,
    color: "var(--cm-syntax-comment)",
    fontStyle: "italic",
  },
]);

export const templateExtensions = [
  templateTheme,
  syntaxHighlighting(templateHighlightStyle),
];
