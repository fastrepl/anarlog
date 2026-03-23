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
  type NodeViewComponentProps,
  useEditorEffect,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { Facehash, stringHash } from "facehash";
import {
  Building2Icon,
  MessageSquareIcon,
  StickyNoteIcon,
  UserIcon,
} from "lucide-react";
import {
  NodeSelection,
  Plugin,
  PluginKey,
  TextSelection,
} from "prosemirror-state";
import { forwardRef, useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

import { schema } from "./schema";

const GLOBAL_NAVIGATE_FUNCTION = "__HYPR_NAVIGATE__";

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
// Suggestion plugin
// ---------------------------------------------------------------------------
interface SuggestionState {
  active: boolean;
  query: string;
  from: number;
  to: number;
}

export const mentionSuggestionKey = new PluginKey<SuggestionState>(
  "mentionSuggestion",
);

export function isMentionActive(
  state: import("prosemirror-state").EditorState,
): boolean {
  const pluginState = mentionSuggestionKey.getState(state);
  return pluginState?.active === true;
}

function findSuggestion(
  state: import("prosemirror-state").EditorState,
  trigger: string,
): SuggestionState | null {
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

  return { active: true, query, from, to };
}

export function mentionSuggestionPlugin(trigger: string) {
  return new Plugin<SuggestionState>({
    key: mentionSuggestionKey,
    state: {
      init: () => ({ active: false, query: "", from: 0, to: 0 }),
      apply(tr, prev, _oldState, newState) {
        const meta = tr.getMeta(mentionSuggestionKey);
        if (meta?.deactivate) {
          return { active: false, query: "", from: 0, to: 0 };
        }
        if (tr.docChanged || tr.selectionSet) {
          return (
            findSuggestion(newState, trigger) ?? {
              active: false,
              query: "",
              from: 0,
              to: 0,
            }
          );
        }
        return prev;
      },
    },
    props: {
      handleKeyDown(view, event) {
        const state = mentionSuggestionKey.getState(view.state);
        if (!state?.active) return false;

        if (event.key === "Escape") {
          view.dispatch(
            view.state.tr.setMeta(mentionSuggestionKey, { deactivate: true }),
          );
          return true;
        }

        if (["ArrowUp", "ArrowDown", "Enter"].includes(event.key)) {
          return true;
        }

        return false;
      },
    },
  });
}

// ---------------------------------------------------------------------------
// Mention popup
// ---------------------------------------------------------------------------
const FACEHASH_BG_CLASSES = [
  "bg-amber-50",
  "bg-rose-50",
  "bg-violet-50",
  "bg-blue-50",
  "bg-teal-50",
  "bg-green-50",
  "bg-cyan-50",
  "bg-fuchsia-50",
  "bg-indigo-50",
  "bg-yellow-50",
];

function getMentionFacehashBgClass(name: string) {
  const hash = stringHash(name);
  return FACEHASH_BG_CLASSES[hash % FACEHASH_BG_CLASSES.length];
}

function MentionAvatar({
  id,
  type,
  label,
}: {
  id: string;
  type: string;
  label: string;
}) {
  if (type === "human") {
    const facehashName = label || id || "?";
    const bgClass = getMentionFacehashBgClass(facehashName);
    return (
      <span className={cn(["mention-avatar", bgClass])}>
        <Facehash
          name={facehashName}
          size={16}
          showInitial={true}
          interactive={false}
          colorClasses={[bgClass]}
        />
      </span>
    );
  }

  const Icon =
    type === "session"
      ? StickyNoteIcon
      : type === "organization"
        ? Building2Icon
        : type === "chat_shortcut"
          ? MessageSquareIcon
          : UserIcon;

  return (
    <span className="mention-avatar mention-avatar-icon">
      <Icon className="mention-inline-icon" />
    </span>
  );
}

export function MentionSuggestion({ config }: { config: MentionConfig }) {
  const [items, setItems] = useState<MentionItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [active, setActive] = useState(false);
  const [query, setQuery] = useState("");
  const popupRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  const insertMention = useEditorEventCallback((view, item: MentionItem) => {
    if (!view) return;
    const state = mentionSuggestionKey.getState(view.state);
    if (!state?.active) return;

    const mentionNode = schema.nodes["mention-@"].create({
      id: item.id,
      type: item.type,
      label: item.label,
    });
    const space = schema.text(" ");

    const tr = view.state.tr
      .replaceWith(state.from, state.to, [mentionNode, space])
      .setMeta(mentionSuggestionKey, { deactivate: true });

    view.dispatch(tr);
    view.focus();
  });

  useEditorEffect((view) => {
    if (!view) return;
    const state = mentionSuggestionKey.getState(view.state);
    const isActive = state?.active ?? false;

    setActive(isActive);
    setQuery(state?.query ?? "");

    if (!isActive) {
      cleanupRef.current?.();
      cleanupRef.current = null;
      return;
    }

    const popup = popupRef.current;
    if (!popup) return;

    const coords = view.coordsAtPos(state!.from);
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
      .handleSearch(query)
      .then((results) => {
        setItems(results.slice(0, 5));
        setSelectedIndex(0);
      })
      .catch(() => {
        setItems([]);
      });
  }, [active, query, config]);

  useEffect(() => {
    if (!active) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex(
          (prev) => (prev + items.length - 1) % Math.max(items.length, 1),
        );
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % Math.max(items.length, 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const item = items[selectedIndex];
        if (item) insertMention(item);
      }
    };

    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [active, items, selectedIndex, insertMention]);

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
// Mention node view
// ---------------------------------------------------------------------------
export const MentionNodeView = forwardRef<HTMLElement, NodeViewComponentProps>(
  ({ nodeProps, ...htmlAttrs }, ref) => {
    const { node } = nodeProps;
    const { id, type, label } = node.attrs;
    const mentionId = String(id ?? "");
    const mentionType = String(type ?? "");
    const mentionLabel = String(label ?? "");
    const MAX_MENTION_LENGTH = 20;
    const displayLabel =
      mentionLabel.length > MAX_MENTION_LENGTH
        ? mentionLabel.slice(0, MAX_MENTION_LENGTH) + "…"
        : mentionLabel;
    const path = `/app/${mentionType}/${mentionId}`;

    const handleClick = useCallback(
      (e: React.MouseEvent) => {
        e.preventDefault();
        const navigate = (window as any)[GLOBAL_NAVIGATE_FUNCTION];
        if (navigate) navigate(path);
      },
      [path],
    );

    return (
      <span ref={ref as any} {...htmlAttrs}>
        <a
          className="mention"
          data-mention="true"
          data-id={mentionId}
          data-type={mentionType}
          data-label={mentionLabel}
          href="javascript:void(0)"
          onClick={handleClick}
        >
          <MentionAvatar
            id={mentionId}
            type={mentionType}
            label={mentionLabel}
          />
          <span className="mention-text">{displayLabel}</span>
        </a>
      </span>
    );
  },
);

MentionNodeView.displayName = "MentionNodeView";

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
