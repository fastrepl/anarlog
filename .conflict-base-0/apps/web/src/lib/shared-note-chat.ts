import {
  getSharedNotePlainText,
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "./shared-notes.ts";

const MAX_NOTE_CONTEXT_CHARS = 24_000;

export type SharedNoteChatMessage = {
  role: "user" | "assistant";
  content: string;
};

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
  const rest = lines.pop() ?? "";
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
        ...messages,
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
      onDelta(delta);
    }
    if (result.done) {
      return;
    }
  }
  const tail = parseSseLine(buffer + decoder.decode());
  if (tail.type === "delta") {
    onDelta(tail.content);
  }
}
