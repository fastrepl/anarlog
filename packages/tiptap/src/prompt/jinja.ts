import {
  type Completion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
} from "@codemirror/autocomplete";
import { closePercentBrace, jinja } from "@codemirror/lang-jinja";
import { type Diagnostic, linter } from "@codemirror/lint";
import { type Extension, RangeSetBuilder } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";

function filterCompletionSource(filters: string[]): CompletionSource {
  const filterCompletions: Completion[] = filters.map((f) => ({
    label: f,
    type: "function",
    detail: "filter",
  }));

  return (context: CompletionContext): CompletionResult | null => {
    const { state, pos } = context;
    const textBefore = state.sliceDoc(Math.max(0, pos - 50), pos);

    const pipeMatch = textBefore.match(/\|\s*(\w*)$/);
    if (pipeMatch) {
      const word = context.matchBefore(/\w*/);
      return {
        from: word?.from ?? pos,
        options: filterCompletions,
        validFor: /^\w*$/,
      };
    }

    return null;
  };
}

export function jinjaLanguage(
  variables: string[],
  filters: string[],
): Extension[] {
  const variableCompletions: Completion[] = variables.map((v) => ({
    label: v,
    type: "variable",
  }));

  const jinjaSupport = jinja({
    variables: variableCompletions,
  });

  const exts: Extension[] = [jinjaSupport, closePercentBrace];

  if (filters.length > 0) {
    exts.push(
      jinjaSupport.language.data.of({
        autocomplete: filterCompletionSource(filters),
      }),
    );
  }

  return exts;
}

const readonlyMark = Decoration.mark({ class: "cm-readonly-region" });

export function readonlyVisuals(
  getRanges: () => Array<{ from: number; to: number }>,
): Extension {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = this.buildDecorations(view);
      }

      buildDecorations(_view: EditorView): DecorationSet {
        const builder = new RangeSetBuilder<Decoration>();
        const ranges = getRanges().sort((a, b) => a.from - b.from);

        for (const { from, to } of ranges) {
          builder.add(from, to, readonlyMark);
        }

        return builder.finish();
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged) {
          this.decorations = this.buildDecorations(update.view);
        }
      }
    },
    { decorations: (v) => v.decorations },
  );
}

const STATEMENT_REGEX = /\{%[\s\S]*?%\}/g;

interface JinjaBlock {
  type: "if" | "for" | "block" | "macro";
  keyword: string;
  start: number;
  end: number;
}

export function jinjaLinter(): Extension {
  return linter((view) => {
    const diagnostics: Diagnostic[] = [];
    const doc = view.state.doc.toString();

    let pos = 0;

    while (pos < doc.length) {
      if (doc.slice(pos, pos + 2) === "{{") {
        const endPos = doc.indexOf("}}", pos + 2);
        if (endPos === -1) {
          diagnostics.push({
            from: pos,
            to: pos + 2,
            severity: "error",
            message: "Unclosed expression: missing }}",
          });
          break;
        }
        pos = endPos + 2;
        continue;
      }

      if (doc.slice(pos, pos + 2) === "{%") {
        const endPos = doc.indexOf("%}", pos + 2);
        if (endPos === -1) {
          diagnostics.push({
            from: pos,
            to: pos + 2,
            severity: "error",
            message: "Unclosed statement: missing %}",
          });
          break;
        }
        pos = endPos + 2;
        continue;
      }

      if (doc.slice(pos, pos + 2) === "{#") {
        const endPos = doc.indexOf("#}", pos + 2);
        if (endPos === -1) {
          diagnostics.push({
            from: pos,
            to: pos + 2,
            severity: "error",
            message: "Unclosed comment: missing #}",
          });
          break;
        }
        pos = endPos + 2;
        continue;
      }

      pos++;
    }

    const blockStack: JinjaBlock[] = [];

    const openingKeywords = ["if", "for", "block", "macro"];
    const closingKeywords = ["endif", "endfor", "endblock", "endmacro"];

    for (const match of doc.matchAll(STATEMENT_REGEX)) {
      if (match.index === undefined) continue;

      const content = match[0].slice(2, -2).trim();
      const parts = content.split(/\s+/);
      const keyword = parts[0];

      if (openingKeywords.includes(keyword)) {
        blockStack.push({
          type: keyword as JinjaBlock["type"],
          keyword,
          start: match.index,
          end: match.index + match[0].length,
        });
      } else if (closingKeywords.includes(keyword)) {
        const expectedOpening = keyword.slice(3);
        const lastBlock = blockStack.pop();

        if (!lastBlock) {
          diagnostics.push({
            from: match.index,
            to: match.index + match[0].length,
            severity: "error",
            message: `Unexpected ${keyword}: no matching opening block`,
          });
        } else if (lastBlock.type !== expectedOpening) {
          diagnostics.push({
            from: match.index,
            to: match.index + match[0].length,
            severity: "error",
            message: `Mismatched block: expected end${lastBlock.type}, found ${keyword}`,
          });
        }
      } else if (keyword === "elif" || keyword === "else") {
        if (
          blockStack.length === 0 ||
          blockStack[blockStack.length - 1].type !== "if"
        ) {
          diagnostics.push({
            from: match.index,
            to: match.index + match[0].length,
            severity: "error",
            message: `${keyword} outside of if block`,
          });
        }
      }
    }

    for (const unclosed of blockStack) {
      diagnostics.push({
        from: unclosed.start,
        to: unclosed.end,
        severity: "error",
        message: `Unclosed ${unclosed.keyword} block: missing end${unclosed.type}`,
      });
    }

    return diagnostics;
  });
}
