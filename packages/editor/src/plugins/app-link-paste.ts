import { Plugin, PluginKey } from "prosemirror-state";

import { parseAppLinkUrl } from "../app-link";

export function appLinkPastePlugin() {
  return new Plugin({
    key: new PluginKey("appLinkPaste"),
    props: {
      handlePaste(view, event) {
        const nodeType = view.state.schema.nodes.appLink;
        if (!nodeType) {
          return false;
        }

        const text = event.clipboardData?.getData("text/plain") ?? "";
        const trimmed = text.trim();

        if (!trimmed || /\s/.test(trimmed)) {
          return false;
        }

        const attrs = parseAppLinkUrl(trimmed);
        if (!attrs) {
          return false;
        }

        const { from, to } = view.state.selection;
        const node = nodeType.create(attrs);
        const space = view.state.schema.text(" ");
        const tr = view.state.tr.replaceWith(from, to, [node, space]);

        view.dispatch(tr);
        return true;
      },
    },
  });
}
