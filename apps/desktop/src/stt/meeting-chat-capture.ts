import { parseJsonContent } from "@hypr/editor/markdown";
import type { JSONContent } from "@hypr/editor/note";
import { commands as detectCommands } from "@hypr/plugin-detect";
import type { MeetingCapturedChatMessage } from "@hypr/plugin-detect";

import {
  appendRawNoteParagraphs,
  getRawNoteEditorContent,
} from "~/editor-bridge/raw-note-registry";
import { appendSessionRawNote, updateSession } from "~/session/queries";

const MEETING_CHAT_CAPTURE_INTERVAL_MS = 5_000;

export function appendCapturedMeetingChatMessagesToRawMd(
  rawMd: string | undefined,
  messages: MeetingCapturedChatMessage[],
  seenSignatures: ReadonlySet<string>,
) {
  const doc = parseJsonContent(rawMd);
  const content = [...(doc.content ?? [])];
  const existingLines = new Set(content.map(extractNoteText));
  const pendingSignatures = new Set<string>();
  const paragraphs: JSONContent[] = [];
  const processedSignatures: string[] = [];

  for (const message of messages) {
    const signature = getCapturedMeetingChatSignature(message);
    if (seenSignatures.has(signature) || pendingSignatures.has(signature)) {
      continue;
    }

    const paragraph = buildCapturedMeetingChatParagraph(message);
    const line = extractNoteText(paragraph);
    if (existingLines.has(line)) {
      processedSignatures.push(signature);
      continue;
    }

    pendingSignatures.add(signature);
    paragraphs.push(paragraph);
    processedSignatures.push(signature);
  }

  if (paragraphs.length === 0) {
    return {
      rawMd: rawMd ?? JSON.stringify(doc),
      appended: 0,
      paragraphs,
      processedSignatures,
    };
  }

  return {
    rawMd: JSON.stringify({
      ...doc,
      content: [...content, ...paragraphs],
    }),
    appended: paragraphs.length,
    paragraphs,
    processedSignatures,
  };
}

export function startMeetingChatCapture({
  sessionId,
  bundleIds,
}: {
  sessionId: string;
  bundleIds: string[];
}) {
  const seenSignatures = new Set<string>();
  let stopped = false;
  let inFlight = false;

  const capture = async () => {
    if (stopped || inFlight) {
      return;
    }

    inFlight = true;
    try {
      const result = await detectCommands.captureMeetingChatMessages(bundleIds);
      if (stopped) {
        return;
      }
      if (result.status === "error") {
        console.warn("[listener] failed to capture meeting chat", result.error);
        return;
      }
      if (result.data.messages.length === 0) {
        return;
      }

      const processedSignatures = await persistCapturedMeetingChatMessages({
        sessionId,
        messages: result.data.messages,
        seenSignatures,
        isStopped: () => stopped,
      });
      if (stopped) {
        return;
      }
      for (const signature of processedSignatures) {
        seenSignatures.add(signature);
      }
    } catch (error) {
      console.warn("[listener] failed to capture meeting chat", error);
    } finally {
      inFlight = false;
    }
  };

  void capture();
  const interval = setInterval(() => {
    void capture();
  }, MEETING_CHAT_CAPTURE_INTERVAL_MS);

  return () => {
    stopped = true;
    clearInterval(interval);
  };
}

async function persistCapturedMeetingChatMessages({
  sessionId,
  messages,
  seenSignatures,
  isStopped,
}: {
  sessionId: string;
  messages: MeetingCapturedChatMessage[];
  seenSignatures: ReadonlySet<string>;
  isStopped: () => boolean;
}) {
  const editorContent = getRawNoteEditorContent(sessionId);
  if (editorContent) {
    const next = appendCapturedMeetingChatMessagesToRawMd(
      JSON.stringify(editorContent),
      messages,
      seenSignatures,
    );
    if (next.appended === 0) {
      return next.processedSignatures;
    }
    if (isStopped()) {
      return [];
    }

    const editorResult = appendRawNoteParagraphs(sessionId, next.paragraphs);
    if (editorResult.status === "deferred") {
      return [];
    }
    if (editorResult.status === "updated") {
      try {
        await updateSession(sessionId, { raw_md: editorResult.rawMd });
      } catch (error) {
        console.warn(
          "[listener] failed to persist captured meeting chat",
          error,
        );
      }
      return next.processedSignatures;
    }
  }

  let processedSignatures: string[] = [];
  await appendSessionRawNote(sessionId, (rawMd) => {
    if (isStopped()) {
      return null;
    }

    const next = appendCapturedMeetingChatMessagesToRawMd(
      rawMd,
      messages,
      seenSignatures,
    );
    processedSignatures = next.processedSignatures;
    return next.appended > 0 ? next.rawMd : null;
  });
  return processedSignatures;
}

function buildCapturedMeetingChatParagraph(
  message: MeetingCapturedChatMessage,
): JSONContent {
  const platform = formatMeetingPlatform(message.platform);
  const metadata = [message.timestamp, message.sender]
    .filter((value): value is string => typeof value === "string" && !!value)
    .join(" ");
  const prefix = metadata
    ? `[${platform} chat] ${metadata}: `
    : `[${platform} chat] `;

  return {
    type: "paragraph",
    content: [
      { type: "text", text: prefix },
      ...buildLinkedText(message.text, message.links),
    ],
  };
}

function buildLinkedText(text: string, links: string[]): JSONContent[] {
  const uniqueLinks = [...new Set(links.filter(Boolean))];
  const content: JSONContent[] = [];
  let cursor = 0;

  while (cursor < text.length) {
    const nextLink = uniqueLinks
      .map((link) => ({ link, index: text.indexOf(link, cursor) }))
      .filter(({ index }) => index >= 0)
      .sort((left, right) => left.index - right.index)[0];

    if (!nextLink) {
      content.push({ type: "text", text: text.slice(cursor) });
      break;
    }
    if (nextLink.index > cursor) {
      content.push({
        type: "text",
        text: text.slice(cursor, nextLink.index),
      });
    }
    content.push({
      type: "text",
      text: nextLink.link,
      marks: [{ type: "link", attrs: { href: nextLink.link } }],
    });
    cursor = nextLink.index + nextLink.link.length;
  }

  return content.length > 0 ? content : [{ type: "text", text }];
}

function formatMeetingPlatform(
  platform: MeetingCapturedChatMessage["platform"],
) {
  switch (platform) {
    case "zoom":
      return "Zoom";
    case "slack":
      return "Slack";
    default:
      return "Meeting";
  }
}

function getCapturedMeetingChatSignature(message: MeetingCapturedChatMessage) {
  return message.id
    ? [message.platform, message.surface, message.id].join("\n")
    : [
        message.platform,
        message.surface,
        message.sender ?? "",
        message.timestamp ?? "",
        message.text,
      ].join("\n");
}

function extractNoteText(node: JSONContent): string {
  return [
    node.text ?? "",
    ...(node.content?.map((child) => extractNoteText(child)) ?? []),
  ].join("");
}
