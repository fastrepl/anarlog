export type SlackLinkKind = "channel" | "message" | "thread";

export interface SlackAttrs {
  provider: "slack";
  kind: SlackLinkKind;
  url: string;
  workspace: string;
  channelId: string;
  messageTs?: string;
  threadTs?: string;
}

export function getSlackDisplayParts(attrs: SlackAttrs): {
  header: string;
  subline: string;
} {
  switch (attrs.kind) {
    case "channel":
      return { header: attrs.workspace, subline: `#${attrs.channelId}` };
    case "message":
      return {
        header: attrs.workspace,
        subline: `Message in #${attrs.channelId}`,
      };
    case "thread":
      return {
        header: attrs.workspace,
        subline: `Thread in #${attrs.channelId}`,
      };
  }
}
