export type DiscordLinkKind = "channel" | "message" | "invite";

export interface DiscordAttrs {
  provider: "discord";
  kind: DiscordLinkKind;
  url: string;
  guildId?: string;
  channelId?: string;
  messageId?: string;
  inviteCode?: string;
}

export function getDiscordDisplayParts(attrs: DiscordAttrs): {
  header: string;
  subline: string;
} {
  switch (attrs.kind) {
    case "invite":
      return { header: "Discord", subline: `Invite ${attrs.inviteCode}` };
    case "channel":
      return { header: "Discord", subline: "Channel" };
    case "message":
      return { header: "Discord", subline: "Message" };
  }
}
