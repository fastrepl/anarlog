import type { NodeSpec } from "prosemirror-model";

import { getAppLinkLabel, type AppLinkAttrs } from "../app-link";

export const appLinkNodeSpec: NodeSpec = {
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  attrs: {
    provider: { default: "github" },
    kind: { default: null },
    url: { default: null },
    owner: { default: null },
    repo: { default: null },
    number: { default: null },
    subId: { default: null },
    workspace: { default: null },
    channelId: { default: null },
    messageTs: { default: null },
    threadTs: { default: null },
    guildId: { default: null },
    messageId: { default: null },
    inviteCode: { default: null },
  },
  parseDOM: [
    {
      tag: 'span[data-type="app-link"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        const numberAttr = el.getAttribute("data-number");

        return {
          provider: el.getAttribute("data-provider") ?? "github",
          kind: el.getAttribute("data-kind"),
          url: el.getAttribute("data-url"),
          owner: el.getAttribute("data-owner"),
          repo: el.getAttribute("data-repo"),
          number: numberAttr ? Number(numberAttr) : null,
          subId: el.getAttribute("data-sub-id"),
          workspace: el.getAttribute("data-workspace"),
          channelId: el.getAttribute("data-channel-id"),
          messageTs: el.getAttribute("data-message-ts"),
          threadTs: el.getAttribute("data-thread-ts"),
          guildId: el.getAttribute("data-guild-id"),
          messageId: el.getAttribute("data-message-id"),
          inviteCode: el.getAttribute("data-invite-code"),
        };
      },
    },
  ],
  toDOM(node) {
    const attrs: Record<string, string> = {
      "data-type": "app-link",
      "data-provider": String(node.attrs.provider ?? "github"),
      "data-kind": String(node.attrs.kind ?? ""),
      "data-url": String(node.attrs.url ?? ""),
    };

    if (node.attrs.owner) attrs["data-owner"] = String(node.attrs.owner);
    if (node.attrs.repo) attrs["data-repo"] = String(node.attrs.repo);
    if (node.attrs.number != null)
      attrs["data-number"] = String(node.attrs.number);
    if (node.attrs.subId) attrs["data-sub-id"] = String(node.attrs.subId);
    if (node.attrs.workspace) {
      attrs["data-workspace"] = String(node.attrs.workspace);
    }
    if (node.attrs.channelId) {
      attrs["data-channel-id"] = String(node.attrs.channelId);
    }
    if (node.attrs.messageTs) {
      attrs["data-message-ts"] = String(node.attrs.messageTs);
    }
    if (node.attrs.threadTs) {
      attrs["data-thread-ts"] = String(node.attrs.threadTs);
    }
    if (node.attrs.guildId) {
      attrs["data-guild-id"] = String(node.attrs.guildId);
    }
    if (node.attrs.messageId) {
      attrs["data-message-id"] = String(node.attrs.messageId);
    }
    if (node.attrs.inviteCode) {
      attrs["data-invite-code"] = String(node.attrs.inviteCode);
    }

    return [
      "span",
      attrs,
      getAppLinkLabel(node.attrs as AppLinkAttrs) ||
        String(node.attrs.url ?? ""),
    ];
  },
};
