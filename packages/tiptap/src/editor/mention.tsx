import {
  autoUpdate,
  computePosition,
  flip,
  limitShift,
  offset,
  shift,
  type VirtualElement,
} from "@floating-ui/dom";
import Mention from "@tiptap/extension-mention";
import {
  type EditorState,
  NodeSelection,
  Plugin,
  PluginKey,
  TextSelection,
} from "@tiptap/pm/state";
import {
  NodeViewWrapper,
  ReactNodeViewRenderer,
  ReactRenderer,
} from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { type SuggestionOptions } from "@tiptap/suggestion";
import { Facehash, stringHash } from "facehash";
import {
  Building2Icon,
  MessageSquareIcon,
  StickyNoteIcon,
  UserIcon,
} from "lucide-react";
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useState,
} from "react";

import { cn } from "@hypr/utils";

const GLOBAL_NAVIGATE_FUNCTION = "__HYPR_NAVIGATE__";

const mentionPluginKeys: PluginKey[] = [];
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

export function isMentionActive(state: EditorState): boolean {
  return mentionPluginKeys.some((key) => {
    const pluginState = key.getState(state);
    return pluginState?.active === true;
  });
}

export interface MentionItem {
  id: string;
  type: string;
  label: string;
  content?: string;
}

// https://github.com/ueberdosis/tiptap/blob/main/demos/src/Nodes/Mention/React/MentionList.jsx
const Component = forwardRef<
  {
    onKeyDown: (props: { event: KeyboardEvent }) => boolean;
  },
  {
    items: MentionItem[];
    command: (item: MentionItem) => void;
    loading?: boolean;
  }
>((props, ref) => {
  const [selectedIndex, setSelectedIndex] = useState(0);

  const selectItem = (index: number) => {
    const item = props.items[index];
    if (item) {
      props.command(item);
    }
  };

  const upHandler = () => {
    setSelectedIndex(
      (prev) => (prev + props.items.length - 1) % props.items.length,
    );
  };

  const downHandler = () => {
    setSelectedIndex((prev) => (prev + 1) % props.items.length);
  };

  const enterHandler = () => {
    selectItem(selectedIndex);
  };

  useEffect(() => setSelectedIndex(0), [props.items]);

  useImperativeHandle(ref, () => ({
    onKeyDown: ({ event }) => {
      if (props.items.length === 0) {
        return false;
      }

      if (
        event.key === "ArrowUp" ||
        event.key === "ArrowDown" ||
        event.key === "Enter"
      ) {
        event.preventDefault();
      }

      if (props.loading) {
        return true;
      }

      switch (event.key) {
        case "ArrowUp":
          upHandler();
          return true;
        case "ArrowDown":
          downHandler();
          return true;
        case "Enter":
          enterHandler();
          return true;
        default:
          return false;
      }
    },
  }));

  if (props.loading) {
    return <div className="mention-container"></div>;
  }

  if (props.items.length === 0) {
    return null;
  }

  return (
    <div className="mention-container">
      {props.items.map((item, index) => {
        return (
          <button
            className={`mention-item ${index === selectedIndex ? "is-selected" : ""}`}
            key={item.id}
            onClick={() => selectItem(index)}
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
        );
      })}
    </div>
  );
});

// https://github.com/ueberdosis/tiptap/blob/main/demos/src/Nodes/Mention/React/suggestion.js
const suggestion = (
  config: MentionConfig,
): Omit<SuggestionOptions, "editor"> => {
  let cachedItems: MentionItem[] = [];
  let loading = false;
  let currentQuery = "";
  let abortController: AbortController | null = null;
  let activeRenderer: ReactRenderer | null = null;

  const updateRendererProps = (
    renderer: ReactRenderer | null,
    items: MentionItem[],
    isLoading: boolean,
  ) => {
    if (!renderer) {
      return;
    }

    try {
      renderer.updateProps({
        items,
        loading: isLoading,
      });
    } catch (e) {
      console.error("Failed to update renderer props:", e);
    }
  };

  const pluginKey = new PluginKey(`mention-${config.trigger}`);
  mentionPluginKeys.push(pluginKey);

  return {
    char: config.trigger,
    pluginKey,
    command: ({ editor, range, props }) => {
      const item = props as MentionItem;
      if (item.content) {
        editor
          .chain()
          .focus()
          .deleteRange(range)
          .insertContent(item.content)
          .run();
      } else {
        editor
          .chain()
          .focus()
          .insertContentAt(range, [
            {
              type: `mention-${config.trigger}`,
              attrs: {
                id: item.id,
                type: item.type,
                label: item.label,
              },
            },
            { type: "text", text: " " },
          ])
          .run();
      }
    },
    items: async ({ query }) => {
      const normalizedQuery = query ?? "";

      if (normalizedQuery === currentQuery && cachedItems.length > 0) {
        return cachedItems;
      }

      currentQuery = normalizedQuery;

      if (abortController) {
        abortController.abort();
      }

      abortController = new AbortController();
      loading = true;

      setTimeout(() => {
        Promise.resolve(config.handleSearch(normalizedQuery))
          .then((items: MentionItem[]) => {
            cachedItems = items.slice(0, 5);
            loading = false;
            updateRendererProps(activeRenderer, cachedItems, false);
          })
          .catch(() => {
            loading = false;
            updateRendererProps(activeRenderer, [], false);
          });
      }, 0);

      return loading
        ? [{ id: "loading", type: "loading", label: "Loading..." }]
        : [];
    },
    render: () => {
      let renderer: ReactRenderer;
      let cleanup: (() => void) | undefined;
      let floatingEl: HTMLElement;
      let referenceEl: VirtualElement;

      const update = () => {
        void computePosition(referenceEl, floatingEl, {
          placement: "bottom-start",
          middleware: [offset(4), flip(), shift({ limiter: limitShift() })],
        }).then(({ x, y }) => {
          Object.assign(floatingEl.style, {
            left: `${x}px`,
            top: `${y}px`,
          });
        });
      };

      return {
        onStart: (props) => {
          renderer = new ReactRenderer(Component, {
            props: {
              ...props,
              loading,
            },
            editor: props.editor,
          });

          activeRenderer = renderer;

          floatingEl = renderer.element as HTMLElement;
          Object.assign(floatingEl.style, {
            position: "absolute",
            top: "0",
            left: "0",
            zIndex: "9999",
          });
          document.body.appendChild(floatingEl);

          if (!props.clientRect) {
            return;
          }

          referenceEl = {
            getBoundingClientRect: () => props.clientRect?.() ?? new DOMRect(),
          };

          cleanup = autoUpdate(referenceEl, floatingEl, update);
          update();
        },

        onUpdate: (props) => {
          renderer.updateProps({
            ...props,
            loading,
          });
          if (props.clientRect) {
            referenceEl.getBoundingClientRect = () =>
              props.clientRect?.() ?? new DOMRect();
          }
          update();
        },

        onKeyDown: (props) => {
          if (props.event.key === "Escape") {
            cleanup?.();
            floatingEl.remove();
            return true;
          }

          // @ts-ignore
          return renderer.ref.onKeyDown(props);
        },

        onExit: () => {
          cachedItems = [];
          loading = false;
          currentQuery = "";
          activeRenderer = null;
          if (abortController) {
            abortController.abort();
            abortController = null;
          }

          cleanup?.();
          floatingEl.remove();
          renderer.destroy();
        },
      };
    },
  };
};

export type MentionConfig = {
  trigger: string;
  handleSearch: (query: string) => Promise<MentionItem[]>;
};
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

function MentionNodeView({ node }: NodeViewProps) {
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
    <NodeViewWrapper
      as="a"
      className="mention"
      data-mention="true"
      data-id={mentionId}
      data-type={mentionType}
      data-label={mentionLabel}
      href="javascript:void(0)"
      onClick={handleClick}
    >
      <MentionAvatar id={mentionId} type={mentionType} label={mentionLabel} />
      <span className="mention-text">{displayLabel}</span>
    </NodeViewWrapper>
  );
}

export const mention = (config: MentionConfig) => {
  return Mention.extend({
    name: `mention-${config.trigger}`,

    renderMarkdown: (node: {
      attrs?: { id?: string; type?: string; label?: string };
    }) => {
      const id = node.attrs?.id ?? "";
      const type = node.attrs?.type ?? "";
      const label = node.attrs?.label ?? "";
      return `<mention data-id="${id}" data-type="${type}" data-label="${label}"></mention>`;
    },

    addAttributes() {
      return {
        id: {
          default: null,
          parseHTML: (element: Element) => element.getAttribute("data-id"),
          renderHTML: (attributes: { id: string }) => ({
            "data-id": attributes.id,
          }),
        },
        type: {
          default: null,
          parseHTML: (element: Element) => element.getAttribute("data-type"),
          renderHTML: (attributes: { type: string }) => ({
            "data-type": attributes.type,
          }),
        },
        label: {
          default: null,
          parseHTML: (element: Element) => element.getAttribute("data-label"),
          renderHTML: (attributes: { label: string }) => ({
            "data-label": attributes.label,
          }),
        },
      };
    },
    parseHTML() {
      return [
        {
          tag: `a.mention[data-mention="true"]`,
        },
        {
          tag: "mention",
        },
      ];
    },
    addKeyboardShortcuts() {
      const skipMention = (direction: "left" | "right") => () => {
        const { state, view } = this.editor;
        const { selection } = state;

        if (
          selection instanceof NodeSelection &&
          selection.node.type.name === this.name
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
        if (node && node.type.name === this.name) {
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
      };

      return {
        ArrowLeft: skipMention("left"),
        ArrowRight: skipMention("right"),
      };
    },
    addProseMirrorPlugins() {
      const parentPlugins = this.parent?.() ?? [];
      const mentionName = this.name;
      return [
        ...parentPlugins,
        new Plugin({
          key: new PluginKey(`${mentionName}-focus-guard`),
          props: {
            handleKeyDown(view, event) {
              if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
                return false;
              }
              if (
                event.shiftKey ||
                event.metaKey ||
                event.ctrlKey ||
                event.altKey
              ) {
                return false;
              }

              const { state } = view;
              const { selection } = state;
              if (!selection.empty) return false;

              const savedPos = selection.$head.pos;
              const forward = event.key === "ArrowDown";

              setTimeout(() => {
                const { state: currentState } = view;
                const currentSelection = currentState.selection;

                if (
                  currentSelection instanceof NodeSelection &&
                  currentSelection.node.type.name === mentionName
                ) {
                  const pos = forward
                    ? currentSelection.to
                    : currentSelection.from;
                  try {
                    view.dispatch(
                      currentState.tr.setSelection(
                        TextSelection.create(currentState.doc, pos),
                      ),
                    );
                  } catch {
                    // best-effort
                  }
                  return;
                }

                if (
                  currentSelection instanceof TextSelection &&
                  currentSelection.empty
                ) {
                  const $head = currentSelection.$head;
                  const nodeAfter = $head.nodeAfter;
                  if (
                    nodeAfter &&
                    nodeAfter.type.name === mentionName &&
                    $head.parentOffset === 0
                  ) {
                    const pos = $head.pos + nodeAfter.nodeSize;
                    try {
                      view.dispatch(
                        currentState.tr.setSelection(
                          TextSelection.create(currentState.doc, pos),
                        ),
                      );
                    } catch {
                      // best-effort
                    }
                    return;
                  }
                }

                if (!view.hasFocus()) {
                  view.focus();
                  const doc = currentState.doc;
                  const $saved = doc.resolve(
                    Math.min(savedPos, doc.content.size),
                  );
                  const depth = $saved.depth;

                  const targetPos = forward
                    ? Math.min($saved.end(depth) + 1, doc.content.size)
                    : Math.max($saved.start(depth) - 1, 0);

                  try {
                    const $target = doc.resolve(targetPos);
                    const sel = TextSelection.near($target, forward ? 1 : -1);
                    view.dispatch(currentState.tr.setSelection(sel));
                  } catch {
                    // recovery failed, at least focus is restored
                  }
                }
              }, 0);

              return false;
            },
          },
        }),
      ];
    },
    addNodeView() {
      return ReactNodeViewRenderer(MentionNodeView, { as: "span" });
    },
  }).configure({
    deleteTriggerWithBackspace: true,
    suggestion: suggestion(config),
    renderHTML: ({ node }) => {
      const {
        attrs: { id, type, label },
      } = node;
      const path = `/app/${type}/${id}`;
      const initial = label ? label[0].toUpperCase() : "?";

      return [
        "a",
        {
          class: "mention",
          "data-mention": `true`,
          "data-id": id,
          "data-type": type,
          "data-label": label,
          "data-initial": initial,
          href: "javascript:void(0)",
          onclick: `event.preventDefault(); if (window.${GLOBAL_NAVIGATE_FUNCTION}) window.${GLOBAL_NAVIGATE_FUNCTION}('${path}');`,
        },
        ["span", { class: "mention-avatar", "data-initial": initial }],
        ["span", { class: "mention-text" }, label],
      ];
    },
    HTMLAttributes: {
      class: "mention",
    },
  });
};
