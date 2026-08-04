import {
  getSharedNotePlainText,
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "./shared-notes.ts";

const MAX_NOTE_CONTEXT_CHARS = 24_000;
export const MAX_SHARED_NOTE_CHAT_MESSAGES = 40;
export const MAX_SHARED_NOTE_CHAT_MESSAGE_CHARS = 8_000;
export const MAX_SHARED_NOTE_CHAT_CONTEXT_CHARS = 64_000;
export const MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS = 32_000;
export const MAX_SHARED_NOTE_SSE_BUFFER_CHARS = 64_000;

export type SharedNoteChatMessage = {
  role: "user" | "assistant";
  content: string;
};

function maxMessageChars(role: SharedNoteChatMessage["role"]) {
  return role === "assistant"
    ? MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS
    : MAX_SHARED_NOTE_CHAT_MESSAGE_CHARS;
}

function sliceWithoutSplittingSurrogate(value: string, maxChars: number) {
  if (value.length <= maxChars) return value;
  let result = value.slice(0, maxChars);
  const last = result.charCodeAt(result.length - 1);
  if (last >= 0xd800 && last <= 0xdbff) {
    result = result.slice(0, -1);
  }
  return result;
}

export function boundSharedNoteChatMessages(
  messages: SharedNoteChatMessage[],
): SharedNoteChatMessage[] {
  const bounded: SharedNoteChatMessage[] = [];
  let remainingChars = MAX_SHARED_NOTE_CHAT_CONTEXT_CHARS;

  for (
    let index = messages.length - 1;
    index >= 0 && bounded.length < MAX_SHARED_NOTE_CHAT_MESSAGES;
    index -= 1
  ) {
    if (remainingChars === 0) break;
    const message = messages[index];
    const content = sliceWithoutSplittingSurrogate(
      message.content,
      Math.min(maxMessageChars(message.role), remainingChars),
    );
    bounded.unshift({ role: message.role, content });
    remainingChars -= content.length;
  }

  return bounded;
}

export function appendSharedNoteChatMessage(
  messages: SharedNoteChatMessage[],
  message: SharedNoteChatMessage,
) {
  const boundedMessage = {
    ...message,
    content: sliceWithoutSplittingSurrogate(
      message.content,
      maxMessageChars(message.role),
    ),
  };
  return [...messages, boundedMessage].slice(-MAX_SHARED_NOTE_CHAT_MESSAGES);
}

export function appendSharedNoteChatResponse(current: string, delta: string) {
  if (current.length >= MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS) return current;
  return sliceWithoutSplittingSurrogate(
    `${current}${delta}`,
    MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS,
  );
}

export class SharedNoteChatError extends Error {
  status: number;

  constructor(status: number) {
    super(`Shared note chat request failed with status ${status}`);
    this.name = "SharedNoteChatError";
    this.status = status;
  }
}

export function buildSharedNoteChatSystemPrompt(snapshot: SharedNoteSnapshot) {
  const body = withoutDuplicateLeadingTitle(snapshot.body, snapshot.title);
  const text = getSharedNotePlainText(body);
  const noteText =
    text.length > MAX_NOTE_CONTEXT_CHARS
      ? `${text.slice(0, MAX_NOTE_CONTEXT_CHARS)}[truncated]`
      : text;
  return [
    "You are a helpful assistant answering questions about one shared note.",
    "Answer using only the note content below. If the note does not contain the answer, say so instead of guessing.",
    "Keep answers concise. Use Markdown formatting when it helps readability.",
    "",
    `Title: ${snapshot.title || "Untitled note"}`,
    "Content:",
    noteText,
  ].join("\n");
}

export function parseSseLine(
  line: string,
): { type: "delta"; content: string } | { type: "done" } | { type: "none" } {
  const trimmed = line.trim();
  if (!trimmed.startsWith("data:")) {
    return { type: "none" };
  }
  const payload = trimmed.slice("data:".length).trim();
  if (payload === "[DONE]") {
    return { type: "done" };
  }
  if (!payload) {
    return { type: "none" };
  }
  try {
    const parsed = JSON.parse(payload) as {
      choices?: Array<{ delta?: { content?: unknown } }>;
    };
    const content = parsed.choices?.[0]?.delta?.content;
    return typeof content === "string" && content.length > 0
      ? { type: "delta", content }
      : { type: "none" };
  } catch {
    return { type: "none" };
  }
}

export function feedSseChunk(buffer: string, chunk: string) {
  const lines = `${buffer}${chunk}`.split("\n");
  const rest = sliceWithoutSplittingSurrogate(
    lines.pop() ?? "",
    MAX_SHARED_NOTE_SSE_BUFFER_CHARS,
  );
  const deltas: string[] = [];
  let done = false;
  for (const line of lines) {
    const event = parseSseLine(line);
    if (event.type === "done") {
      done = true;
      break;
    }
    if (event.type === "delta") {
      deltas.push(event.content);
    }
  }
  return { buffer: rest, deltas, done };
}

export async function streamSharedNoteChat({
  messages,
  onDelta,
  signal,
  snapshot,
}: {
  messages: SharedNoteChatMessage[];
  onDelta: (delta: string) => void;
  signal?: AbortSignal;
  snapshot: SharedNoteSnapshot;
}): Promise<void> {
  // Dynamic imports keep this module loadable under node --test, which cannot
  // resolve the "@/" alias used by the env and Supabase auth modules.
  const [{ env }, { getAccessToken }] = await Promise.all([
    import("@/env"),
    import("@/functions/access-token"),
  ]);
  const token = await getAccessToken();
  const base = env.VITE_API_URL.endsWith("/")
    ? env.VITE_API_URL
    : `${env.VITE_API_URL}/`;
  const response = await fetch(new URL("chat/completions", base), {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      "x-char-task": "chat",
    },
    body: JSON.stringify({
      model: "auto",
      stream: true,
      messages: [
        { role: "system", content: buildSharedNoteChatSystemPrompt(snapshot) },
        ...boundSharedNoteChatMessages(messages),
      ],
    }),
    signal,
  });
  if (!response.ok || !response.body) {
    throw new SharedNoteChatError(response.status);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let emittedChars = 0;
  const emitDelta = (delta: string) => {
    const remainingChars = MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS - emittedChars;
    if (remainingChars <= 0) return;
    const boundedDelta = sliceWithoutSplittingSurrogate(delta, remainingChars);
    emittedChars += boundedDelta.length;
    if (boundedDelta) onDelta(boundedDelta);
  };
  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    const result = feedSseChunk(
      buffer,
      decoder.decode(value, { stream: true }),
    );
    buffer = result.buffer;
    for (const delta of result.deltas) {
      emitDelta(delta);
    }
    if (result.done) {
      return;
    }
  }
  const tail = parseSseLine(buffer + decoder.decode());
  if (tail.type === "delta") {
    emitDelta(tail.content);
  }
}
