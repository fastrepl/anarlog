import { Plugin, PluginKey } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";

export type LinkOpenHandler = (href: string) => Promise<void> | void;

const OPENABLE_PROTOCOLS = new Set(["http:", "https:"]);

export function getOpenableHref(href: string | null): string | null {
  if (!href) {
    return null;
  }

  try {
    const url = new URL(href);
    if (!OPENABLE_PROTOCOLS.has(url.protocol)) {
      return null;
    }

    return url.href;
  } catch {
    return null;
  }
}

function getClickedAnchor(view: EditorView, event: Event) {
  const target = event.target;
  const element =
    target instanceof Element
      ? target
      : target instanceof Node
        ? target.parentElement
        : null;
  const anchor = element?.closest<HTMLAnchorElement>("a[href]");

  return anchor && view.dom.contains(anchor) ? anchor : null;
}

function isMentionAnchor(anchor: HTMLAnchorElement) {
  return anchor.dataset.mention === "true";
}

export function linkOpenPlugin(onOpen: LinkOpenHandler) {
  return new Plugin({
    key: new PluginKey("linkOpen"),
    props: {
      handleDOMEvents: {
        click(view, event) {
          const anchor = getClickedAnchor(view, event);
          if (!anchor) {
            return false;
          }
          if (isMentionAnchor(anchor)) {
            return false;
          }

          event.preventDefault();
          event.stopPropagation();

          const href = getOpenableHref(anchor.getAttribute("href"));
          if (href) {
            void onOpen(href);
          }

          return true;
        },
      },
    },
  });
}
