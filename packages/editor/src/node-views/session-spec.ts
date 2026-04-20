import type { NodeSpec } from "prosemirror-model";

import { getOptionalTaskStatus } from "../tasks";

export const sessionNodeSpec: NodeSpec = {
  group: "block",
  content: "paragraph",
  marks: "",
  defining: true,
  isolating: true,
  selectable: false,
  attrs: {
    sessionId: { default: null },
    status: { default: null },
    checked: { default: null },
  },
  parseDOM: [
    {
      tag: 'div[data-type="session"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        const status = getOptionalTaskStatus(
          el.getAttribute("data-status"),
          el.getAttribute("data-checked") === "true"
            ? true
            : el.getAttribute("data-checked") === "false"
              ? false
              : undefined,
        );

        return {
          sessionId: el.getAttribute("data-session-id"),
          status,
          checked: status === null ? null : status === "done",
        };
      },
    },
  ],
  toDOM(node) {
    const status = getOptionalTaskStatus(node.attrs.status, node.attrs.checked);
    return [
      "div",
      {
        "data-type": "session",
        "data-session-id": node.attrs.sessionId,
        "data-status": status ?? undefined,
        "data-checked": status ? String(status === "done") : undefined,
      },
      0,
    ];
  },
};
