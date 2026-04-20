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

const CHANNEL_HOSTS = new Set([
  "discord.com",
  "www.discord.com",
  "discordapp.com",
  "www.discordapp.com",
]);
const INVITE_HOSTS = new Set(["discord.gg", "www.discord.gg"]);

function build(
  url: URL,
  attrs: Omit<DiscordAttrs, "provider" | "url">,
): DiscordAttrs {
  return { provider: "discord", url: url.toString(), ...attrs };
}

export function parseDiscordUrl(rawUrl: string): DiscordAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  const hostname = url.hostname.toLowerCase();

  if (INVITE_HOSTS.has(hostname)) {
    const segments = url.pathname.split("/").filter(Boolean);
    const [inviteCode] = segments;

    if (!inviteCode) {
      return null;
    }

    return build(url, { kind: "invite", inviteCode });
  }

  if (!CHANNEL_HOSTS.has(hostname)) {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);

  if (segments[0] === "invite" && segments[1]) {
    return build(url, { kind: "invite", inviteCode: segments[1] });
  }

  if (segments[0] !== "channels" || !segments[1] || !segments[2]) {
    return null;
  }

  const guildId = segments[1];
  const channelId = segments[2];
  const messageId = segments[3];

  if (messageId) {
    return build(url, { kind: "message", guildId, channelId, messageId });
  }

  return build(url, { kind: "channel", guildId, channelId });
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
