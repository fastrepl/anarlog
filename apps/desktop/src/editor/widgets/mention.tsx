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
  Building2Icon,
  MessageSquareIcon,
  StickyNoteIcon,
  UserIcon,
} from "lucide-react";
import {
  type EditorState,
  NodeSelection,
  Plugin,
  PluginKey,
  TextSelection,
} from "prosemirror-state";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export interface MentionItem {
  id: string;
  type: string;
  label: string;
  content?: string;
}

export type MentionConfig = {
  trigger: string;
  handleSearch: (query: string) => Promise<MentionItem[]>;
};

// ---------------------------------------------------------------------------
// Derive mention state from EditorState (no plugin needed)
// ---------------------------------------------------------------------------
interface MentionState {
  query: string;
  from: number;
  to: number;
}

export function findMention(
  state: EditorState,
  trigger: string,
): MentionState | null {
  const { $from } = state.selection;
  if (!state.selection.empty) return null;

  const textBefore = $from.parent.textBetween(
    0,
    $from.parentOffset,
    undefined,
    "\ufffc",
  );

  const triggerIndex = textBefore.lastIndexOf(trigger);
  if (triggerIndex === -1) return null;
  if (triggerIndex > 0 && !/\s/.test(textBefore[triggerIndex - 1])) return null;

  const query = textBefore.slice(triggerIndex + trigger.length);
  if (/\s/.test(query)) return null;

  const from = $from.start() + triggerIndex;
  const to = $from.pos;

  return { query, from, to };
}

// ---------------------------------------------------------------------------
// Mention popup
// ---------------------------------------------------------------------------
export function MentionSuggestion({ config }: { config: MentionConfig }) {
  const [items, setItems] = useState<MentionItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [dismissedFrom, setDismissedFrom] = useState<number | null>(null);
  const popupRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  const editorState = useEditorState();
  const mentionState = editorState
    ? findMention(editorState, config.trigger)
    : null;

  const dismissed =
    mentionState !== null && dismissedFrom === mentionState.from;
  const active = mentionState !== null && !dismissed;

  if (!active && selectedIndex !== 0) {
    setSelectedIndex(0);
  }
  if (mentionState === null && dismissedFrom !== null) {
    setDismissedFrom(null);
  }

  const insertMention = useEditorEventCallback((view, item: MentionItem) => {
    if (!view || !mentionState) return;

    const { schema } = view.state;
    const mentionNode = schema.nodes["mention-@"].create({
      id: item.id,
      type: item.type,
      label: item.label,
    });
    const space = schema.text(" ");

    const tr = view.state.tr.replaceWith(mentionState.from, mentionState.to, [
      mentionNode,
      space,
    ]);

    view.dispatch(tr);
    view.focus();
    setDismissedFrom(mentionState.from);
  });

  useEditorEventListener("keydown", (_view, event) => {
    if (!active || items.length === 0) return false;

    if (event.key === "Escape") {
      if (mentionState) setDismissedFrom(mentionState.from);
      return true;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex(
        (prev) => (prev + items.length - 1) % Math.max(items.length, 1),
      );
      return true;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % Math.max(items.length, 1));
      return true;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      const item = items[selectedIndex];
      if (item) insertMention(item);
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

    const coords = view.coordsAtPos(mentionState!.from);
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

  useEffect(() => {
    if (!active) {
      setItems([]);
      setSelectedIndex(0);
      return;
    }

    config
      .handleSearch(mentionState!.query)
      .then((results) => {
        setItems(results.slice(0, 5));
        setSelectedIndex(0);
      })
      .catch(() => {
        setItems([]);
      });
  }, [active, mentionState?.query, config]);

  if (!active || items.length === 0) return null;

  return createPortal(
    <div
      ref={popupRef}
      className="mention-container"
      style={{ position: "absolute", top: 0, left: 0, zIndex: 9999 }}
    >
      {items.map((item, index) => (
        <button
          key={item.id}
          className={`mention-item ${index === selectedIndex ? "is-selected" : ""}`}
          onClick={() => insertMention(item)}
          onMouseEnter={() => setSelectedIndex(index)}
        >
          {item.type === "session" ? (
            <StickyNoteIcon className="mention-type-icon mention-type-session" />
          ) : item.type === "human" ? (
            <UserIcon className="mention-type-icon mention-type-human" />
          ) : item.type === "organization" ? (
            <Building2Icon className="mention-type-icon mention-type-organization" />
          ) : item.type === "chat_shortcut" ? (
            <MessageSquareIcon className="mention-type-icon mention-type-chat-shortcut" />
          ) : null}
          <span className="mention-label">{item.label}</span>
        </button>
      ))}
    </div>,
    document.body,
  );
}

// ---------------------------------------------------------------------------
// Mention keyboard skip plugin
// ---------------------------------------------------------------------------
export function mentionSkipPlugin() {
  const mentionName = "mention-@";

  return new Plugin({
    key: new PluginKey("mentionSkip"),
    props: {
      handleKeyDown(view, event) {
        if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
          return false;
        }

        const { state } = view;
        const { selection } = state;
        const direction = event.key === "ArrowLeft" ? "left" : "right";

        if (
          selection instanceof NodeSelection &&
          selection.node.type.name === mentionName
        ) {
          const pos = direction === "left" ? selection.from : selection.to;
          view.dispatch(
            state.tr.setSelection(TextSelection.create(state.doc, pos)),
          );
          return true;
        }

        if (!selection.empty) return false;

        const $pos = selection.$head;
        const node = direction === "left" ? $pos.nodeBefore : $pos.nodeAfter;

        if (node && node.type.name === mentionName) {
          const newPos =
            direction === "left"
              ? $pos.pos - node.nodeSize
              : $pos.pos + node.nodeSize;
          view.dispatch(
            state.tr.setSelection(TextSelection.create(state.doc, newPos)),
          );
          return true;
        }

        return false;
      },
    },
  });
}
