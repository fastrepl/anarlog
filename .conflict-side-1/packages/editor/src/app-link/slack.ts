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

const HOSTS = new Set(["app.slack.com"]);

function isWorkspaceHost(hostname: string): boolean {
  return hostname.endsWith(".slack.com") && !HOSTS.has(hostname);
}

function build(
  url: URL,
  attrs: Omit<SlackAttrs, "provider" | "url">,
): SlackAttrs {
  return { provider: "slack", url: url.toString(), ...attrs };
}

export function parseSlackUrl(rawUrl: string): SlackAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  const hostname = url.hostname.toLowerCase();

  if (isWorkspaceHost(hostname)) {
    const workspace = hostname.replace(/\.slack\.com$/, "");
    const segments = url.pathname.split("/").filter(Boolean);
    const [first, channelId, messageId] = segments;

    if (first !== "archives" || !channelId) {
      return null;
    }

    const threadTs = url.searchParams.get("thread_ts") ?? undefined;

    if (messageId && /^p\d+$/.test(messageId)) {
      const kind = threadTs ? "thread" : "message";
      return build(url, {
        kind,
        workspace,
        channelId,
        messageTs: messageId,
        threadTs,
      });
    }

    return build(url, { kind: "channel", workspace, channelId });
  }

  if (HOSTS.has(hostname)) {
    const segments = url.pathname.split("/").filter(Boolean);
    const [first, _teamId, channelId] = segments;

    if (first !== "client" || !channelId) {
      return null;
    }

    return build(url, { kind: "channel", workspace: "app", channelId });
  }

  return null;
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
