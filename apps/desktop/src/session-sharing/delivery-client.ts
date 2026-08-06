import type { Session } from "@supabase/supabase-js";

const MAX_RECAP_BODY_BYTES = 100 * 1024;
const MAX_SLACK_CHANNELS = 200;

export type SlackChannel = {
  id: string;
  name: string;
  isPrivate: boolean;
};

export class ShareDeliveryError extends Error {
  constructor() {
    super("Meeting recap delivery is unavailable");
    this.name = "ShareDeliveryError";
  }
}

export async function sendSessionShareRecapEmail({
  apiBaseUrl,
  session,
  shareId,
  recipients,
  senderName,
  noteTitle,
  noteBody,
  deliveryId,
  signal,
  fetcher = fetch,
}: {
  apiBaseUrl: string;
  session: Session;
  shareId: string;
  recipients: string[];
  senderName: string;
  noteTitle: string;
  noteBody: string;
  deliveryId: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
}) {
  if (
    recipients.length < 1 ||
    recipients.length > 20 ||
    !noteBody.trim() ||
    new TextEncoder().encode(noteBody).length > MAX_RECAP_BODY_BYTES
  ) {
    throw new ShareDeliveryError();
  }
  const response = await fetcher(
    apiUrl(apiBaseUrl, `/shared-notes/${shareId}/recap/email`),
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${session.access_token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        recipients,
        senderName,
        noteTitle,
        noteBody,
        deliveryId,
      }),
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal,
    },
  );
  if (response.status !== 204) throw new ShareDeliveryError();
}

export async function listSlackChannels({
  apiBaseUrl,
  accessToken,
  signal,
  fetcher = fetch,
}: {
  apiBaseUrl: string;
  accessToken: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
}): Promise<SlackChannel[]> {
  const response = await fetcher(
    apiUrl(apiBaseUrl, "/messenger/slack/channels"),
    {
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${accessToken}`,
      },
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal,
    },
  );
  if (!response.ok) throw new ShareDeliveryError();
  const value: unknown = await response.json();
  if (!isRecord(value) || !Array.isArray(value.channels)) {
    throw new ShareDeliveryError();
  }
  if (value.channels.length > MAX_SLACK_CHANNELS) {
    throw new ShareDeliveryError();
  }
  return value.channels.map((channel) => {
    if (
      !isRecord(channel) ||
      typeof channel.id !== "string" ||
      typeof channel.name !== "string" ||
      typeof channel.is_private !== "boolean"
    ) {
      throw new ShareDeliveryError();
    }
    return {
      id: channel.id,
      name: channel.name,
      isPrivate: channel.is_private,
    };
  });
}

export async function sendSlackRecap({
  apiBaseUrl,
  accessToken,
  channel,
  text,
  signal,
  fetcher = fetch,
}: {
  apiBaseUrl: string;
  accessToken: string;
  channel: string;
  text: string;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
}) {
  const response = await fetcher(
    apiUrl(apiBaseUrl, "/messenger/slack/messages"),
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ channel, text }),
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal,
    },
  );
  if (!response.ok) throw new ShareDeliveryError();
}

export function buildSlackRecap({
  senderName,
  noteTitle,
  noteBody,
}: {
  senderName: string;
  noteTitle: string;
  noteBody: string;
}) {
  const signature = `\n\n_Sent by ${senderName} via Anarlog_`;
  const prefix = `*${noteTitle}*\n\n`;
  const bodyBudget = 40_000 - prefix.length - signature.length;
  const body =
    noteBody.length <= bodyBudget
      ? noteBody
      : `${noteBody.slice(0, Math.max(0, bodyBudget - 1)).trimEnd()}…`;
  return `${prefix}${body}${signature}`;
}

function apiUrl(apiBaseUrl: string, path: string) {
  try {
    const base = new URL(apiBaseUrl);
    if (
      !["http:", "https:"].includes(base.protocol) ||
      base.username ||
      base.password ||
      base.search ||
      base.hash
    ) {
      throw new ShareDeliveryError();
    }
    return new URL(path, base.origin);
  } catch (error) {
    if (error instanceof ShareDeliveryError) throw error;
    throw new ShareDeliveryError();
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
