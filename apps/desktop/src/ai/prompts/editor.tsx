import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import readOnlyRangesExtension from "codemirror-readonly-ranges";
import { useCallback, useMemo } from "react";

import { jinjaLanguage, jinjaLinter, readonlyVisuals } from "./jinja";

export interface ReadOnlyRange {
  from: number;
  to: number;
}

interface PromptEditorProps {
  value: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  readOnly?: boolean;
  readOnlyRanges?: ReadOnlyRange[];
  variables?: string[];
  filters?: string[];
}

export function PromptEditor({
  value,
  onChange,
  placeholder,
  readOnly = false,
  readOnlyRanges = [],
  variables = [],
  filters = [],
}: PromptEditorProps) {
  const getReadOnlyRanges = useCallback(
    (_state: EditorState) => {
      if (readOnly || readOnlyRanges.length === 0) {
        return [];
      }

      return readOnlyRanges.map((range) => ({
        from: range.from,
        to: range.to,
      }));
    },
    [readOnly, readOnlyRanges],
  );

  const getRangesForVisuals = useCallback(() => {
    return readOnlyRanges;
  }, [readOnlyRanges]);

  const extensions = useMemo(() => {
    const exts: Extension[] = [
      jinjaLanguage(variables, filters),
      jinjaLinter(),
    ];

    if (!readOnly && readOnlyRanges.length > 0) {
      exts.push(readOnlyRangesExtension(getReadOnlyRanges));
      exts.push(readonlyVisuals(getRangesForVisuals));
    }

    return exts;
  }, [
    readOnly,
    readOnlyRanges,
    getReadOnlyRanges,
    getRangesForVisuals,
    variables,
    filters,
  ]);

  const theme = useMemo(
    () =>
      EditorView.theme({
        "&": {
          height: "100%",
          fontFamily:
            "var(--font-mono, 'Menlo', 'Monaco', 'Courier New', monospace)",
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
        ".cm-placeholder": {
          color: "#999",
          fontStyle: "italic",
        },
        ".cm-readonly-region": {
          backgroundColor: "rgba(0, 0, 0, 0.04)",
          borderRadius: "2px",
        },
        ".cm-tooltip-autocomplete": {
          border: "1px solid #e5e7eb",
          borderRadius: "6px",
          boxShadow: "0 4px 6px -1px rgba(0, 0, 0, 0.1)",
          backgroundColor: "#fff",
        },
        ".cm-tooltip-autocomplete ul li": {
          padding: "4px 8px",
        },
        ".cm-tooltip-autocomplete ul li[aria-selected]": {
          backgroundColor: "#f3f4f6",
        },
        ".cm-diagnostic-error": {
          borderBottom: "2px wavy #ef4444",
        },
        ".cm-lintPoint-error:after": {
          borderBottomColor: "#ef4444",
        },
      }),
    [],
  );

  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      readOnly={readOnly}
      basicSetup={{
        lineNumbers: false,
        foldGutter: false,
        highlightActiveLineGutter: false,
        highlightActiveLine: false,
      }}
      extensions={[theme, ...extensions]}
      height="100%"
    />
  );
}
