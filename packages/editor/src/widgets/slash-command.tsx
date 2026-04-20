import {
  autoUpdate,
  computePosition,
  flip,
  limitShift,
  offset,
  shift,
  type VirtualElement,
} from "@floating-ui/dom";
import {
  useEditorEffect,
  useEditorEventCallback,
  useEditorEventListener,
  useEditorState,
} from "@handlewithcare/react-prosemirror";
import {
  CodeIcon,
  Heading1Icon,
  Heading2Icon,
  Heading3Icon,
  ListIcon,
  ListOrderedIcon,
  ListTodoIcon,
  MinusIcon,
  QuoteIcon,
  TextIcon,
} from "lucide-react";
import { setBlockType } from "prosemirror-commands";
import { wrapInList } from "prosemirror-schema-list";
import type { EditorState, Transaction } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { useRef, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

import { schema } from "../note/schema";
import { createTaskItemAttrs } from "../tasks";

// ---------------------------------------------------------------------------
// Slash command items
// ---------------------------------------------------------------------------
export interface SlashCommandItem {
  id: string;
  label: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
  keywords: string[];
  action: (view: EditorView, from: number, to: number) => void;
}

function clearSlashAndRun(
  view: EditorView,
  from: number,
  to: number,
  command: (
    state: EditorState,
    dispatch?: (tr: Transaction) => void,
  ) => boolean,
) {
  const tr = view.state.tr.delete(from, to);
  view.dispatch(tr);
  command(view.state, (tr) => view.dispatch(tr));
}

const SLASH_COMMANDS: SlashCommandItem[] = [
  {
    id: "paragraph",
    label: "Text",
    description: "Plain text",
    icon: TextIcon,
    keywords: ["text", "paragraph", "plain"],
    action(view, from, to) {
      clearSlashAndRun(view, from, to, setBlockType(schema.nodes.paragraph));
    },
  },
  {
    id: "heading1",
    label: "Heading 1",
    description: "Large heading",
    icon: Heading1Icon,
    keywords: ["heading", "h1", "title", "large"],
    action(view, from, to) {
      clearSlashAndRun(
        view,
        from,
        to,
        setBlockType(schema.nodes.heading, { level: 1 }),
      );
    },
  },
  {
    id: "heading2",
    label: "Heading 2",
    description: "Medium heading",
    icon: Heading2Icon,
    keywords: ["heading", "h2", "subtitle", "medium"],
    action(view, from, to) {
      clearSlashAndRun(
        view,
        from,
        to,
        setBlockType(schema.nodes.heading, { level: 2 }),
      );
    },
  },
  {
    id: "heading3",
    label: "Heading 3",
    description: "Small heading",
    icon: Heading3Icon,
    keywords: ["heading", "h3", "small"],
    action(view, from, to) {
      clearSlashAndRun(
        view,
        from,
        to,
        setBlockType(schema.nodes.heading, { level: 3 }),
      );
    },
  },
  {
    id: "bulletList",
    label: "Bullet List",
    description: "Unordered list",
    icon: ListIcon,
    keywords: ["bullet", "list", "unordered", "ul"],
    action(view, from, to) {
      clearSlashAndRun(view, from, to, wrapInList(schema.nodes.bulletList));
    },
  },
  {
    id: "orderedList",
    label: "Numbered List",
    description: "Ordered list",
    icon: ListOrderedIcon,
    keywords: ["numbered", "list", "ordered", "ol"],
    action(view, from, to) {
      clearSlashAndRun(view, from, to, wrapInList(schema.nodes.orderedList));
    },
  },
  {
    id: "taskList",
    label: "Task List",
    description: "List with checkboxes",
    icon: ListTodoIcon,
    keywords: ["task", "todo", "checkbox", "check"],
    action(view, from, to) {
      const tr = view.state.tr.delete(from, to);
      view.dispatch(tr);
      const taskItem = schema.nodes.taskItem.create(
        createTaskItemAttrs(false),
        schema.nodes.paragraph.create(),
      );
      const taskList = schema.nodes.taskList.create(null, taskItem);
      const { $from } = view.state.selection;
      const blockStart = $from.start($from.depth) - 1;
      const blockEnd = $from.end($from.depth) + 1;
      view.dispatch(view.state.tr.replaceWith(blockStart, blockEnd, taskList));
    },
  },
  {
    id: "blockquote",
    label: "Quote",
    description: "Block quote",
    icon: QuoteIcon,
    keywords: ["quote", "blockquote", "callout"],
    action(view, from, to) {
      clearSlashAndRun(view, from, to, (state, dispatch) => {
        const { $from, $to } = state.selection;
        const range = $from.blockRange($to);
        if (!range) return false;
        if (dispatch) {
          const tr = state.tr.wrap(range, [{ type: schema.nodes.blockquote }]);
          dispatch(tr);
        }
        return true;
      });
    },
  },
  {
    id: "codeBlock",
    label: "Code Block",
    description: "Code with syntax highlighting",
    icon: CodeIcon,
    keywords: ["code", "pre", "block", "snippet"],
    action(view, from, to) {
      clearSlashAndRun(view, from, to, setBlockType(schema.nodes.codeBlock));
    },
  },
  {
    id: "horizontalRule",
    label: "Divider",
    description: "Horizontal rule",
    icon: MinusIcon,
    keywords: ["divider", "horizontal", "rule", "line", "hr"],
    action(view, from, to) {
      const tr = view.state.tr.delete(from, to);
      view.dispatch(tr);
      const hr = schema.nodes.horizontalRule.create();
      const paragraph = schema.nodes.paragraph.create();
      const { $from } = view.state.selection;
      const blockStart = $from.start($from.depth) - 1;
      const blockEnd = $from.end($from.depth) + 1;
      view.dispatch(
        view.state.tr.replaceWith(blockStart, blockEnd, [hr, paragraph]),
      );
    },
  },
];

// ---------------------------------------------------------------------------
// Derive slash command state from EditorState (no plugin needed)
// ---------------------------------------------------------------------------
interface SlashCommandState {
  query: string;
  from: number;
  to: number;
}

function findSlashCommand(state: EditorState): SlashCommandState | null {
  const { $from } = state.selection;
  if (!state.selection.empty) return null;

  const textBefore = $from.parent.textBetween(
    0,
    $from.parentOffset,
    undefined,
    "\ufffc",
  );

  const slashIndex = textBefore.lastIndexOf("/");
  if (slashIndex === -1) return null;
  if (slashIndex > 0 && !/\s/.test(textBefore[slashIndex - 1])) return null;

  const query = textBefore.slice(slashIndex + 1);
  if (/\s/.test(query)) return null;

  const from = $from.start() + slashIndex;
  const to = $from.pos;

  return { query, from, to };
}

function filterCommands(query: string): SlashCommandItem[] {
  if (!query) return SLASH_COMMANDS;
  const q = query.toLowerCase();
  return SLASH_COMMANDS.filter(
    (cmd) =>
      cmd.label.toLowerCase().includes(q) ||
      cmd.keywords.some((kw) => kw.includes(q)),
  );
}

// ---------------------------------------------------------------------------
// React component
// ---------------------------------------------------------------------------
export function SlashCommandMenu() {
  const popupRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [dismissedFrom, setDismissedFrom] = useState<number | null>(null);

  const editorState = useEditorState();
  const slashState = editorState ? findSlashCommand(editorState) : null;

  const dismissed = slashState !== null && dismissedFrom === slashState.from;
  const active = slashState !== null && !dismissed;
  const items = active ? filterCommands(slashState.query) : [];

  if (!active && selectedIndex !== 0) {
    setSelectedIndex(0);
  }
  if (slashState === null && dismissedFrom !== null) {
    setDismissedFrom(null);
  }

  const executeCommand = useEditorEventCallback(
    (view, item: SlashCommandItem) => {
      if (!view || !slashState) return;
      setDismissedFrom(slashState.from);
      item.action(view, slashState.from, slashState.to);
      view.focus();
    },
  );

  useEditorEventListener("keydown", (_view, event) => {
    if (!active || items.length === 0) return false;

    if (event.key === "Escape") {
      if (slashState) {
        setDismissedFrom(slashState.from);
      }
      return true;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((prev) => (prev + items.length - 1) % items.length);
      return true;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % items.length);
      return true;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      const item = items[selectedIndex];
      if (item) executeCommand(item);
      return true;
    }

    return false;
  });

  useEditorEffect((view) => {
    if (!view || !active || items.length === 0) {
      cleanupRef.current?.();
      cleanupRef.current = null;
      return;
    }

    const popup = popupRef.current;
    if (!popup) return;

    const coords = view.coordsAtPos(slashState!.from);
    const referenceEl: VirtualElement = {
      getBoundingClientRect: () =>
        new DOMRect(coords.left, coords.top, 0, coords.bottom - coords.top),
    };

    const update = () => {
      void computePosition(referenceEl, popup, {
        placement: "bottom-start",
        middleware: [offset(4), flip(), shift({ limiter: limitShift() })],
      }).then(({ x, y }) => {
        Object.assign(popup.style, {
          left: `${x}px`,
          top: `${y}px`,
        });
      });
    };

    cleanupRef.current?.();
    cleanupRef.current = autoUpdate(referenceEl, popup, update);
    update();
  });

  if (!active || items.length === 0) return null;

  return createPortal(
    <div
      ref={popupRef}
      className={cn([
        "absolute z-[9999] max-h-80 max-w-[280px] min-w-[220px]",
        "overflow-y-auto rounded-lg bg-white p-1",
        "shadow-[0_0_0_1px_rgba(0,0,0,0.05),0_6px_12px_-3px_rgba(0,0,0,0.08)]",
      ])}
      style={{ top: 0, left: 0 }}
    >
      <div className="px-2 py-1 text-[0.7rem] font-semibold tracking-wide text-neutral-400 uppercase select-none">
        Commands
      </div>
      {items.map((item, index) => (
        <button
          key={item.id}
          className={cn([
            "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left",
            "cursor-pointer border-none bg-transparent transition-colors",
            index === selectedIndex && "bg-neutral-100",
          ])}
          onClick={() => executeCommand(item)}
          onMouseEnter={() => setSelectedIndex(index)}
        >
          <span className="flex size-8 shrink-0 items-center justify-center rounded-md border border-neutral-200 bg-neutral-50">
            <item.icon className="size-4 text-neutral-600" />
          </span>
          <span className="flex flex-col gap-px overflow-hidden">
            <span className="truncate text-[0.85rem] font-medium text-neutral-900">
              {item.label}
            </span>
            <span className="truncate text-xs text-neutral-400">
              {item.description}
            </span>
          </span>
        </button>
      ))}
    </div>,
    document.body,
  );
}
