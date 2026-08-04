import assert from "node:assert/strict";
import test from "node:test";

import {
  appendSharedNoteChatMessage,
  appendSharedNoteChatResponse,
  boundSharedNoteChatMessages,
  buildSharedNoteChatSystemPrompt,
  feedSseChunk,
  MAX_SHARED_NOTE_CHAT_CONTEXT_CHARS,
  MAX_SHARED_NOTE_CHAT_MESSAGES,
  MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS,
  MAX_SHARED_NOTE_SSE_BUFFER_CHARS,
  parseSseLine,
  type SharedNoteChatMessage,
  SharedNoteChatError,
} from "./shared-note-chat.ts";
import type { SharedNoteSnapshot } from "./shared-notes.ts";

function makeSnapshot(bodyText: string): SharedNoteSnapshot {
  return {
    shareId: "82a163dd-d595-45f8-8d71-cf38bbb1ce12",
    schemaVersion: 1,
    contentRevision: 1,
    title: "Weekly sync",
    body: {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: bodyText }],
        },
      ],
    },
    attachments: [],
    publishedAt: "2026-07-01T00:00:00.000Z",
  };
}

test("system prompt embeds the note title and body text", () => {
  const prompt = buildSharedNoteChatSystemPrompt(
    makeSnapshot("Decisions and next steps."),
  );

  assert.ok(prompt.includes("Title: Weekly sync"));
  assert.ok(prompt.includes("Decisions and next steps."));
  assert.ok(!prompt.includes("[truncated]"));
});

test("system prompt truncates long note content with a suffix", () => {
  const prompt = buildSharedNoteChatSystemPrompt(
    makeSnapshot("x".repeat(30_000)),
  );

  assert.ok(prompt.includes(`${"x".repeat(24_000)}[truncated]`));
  assert.ok(!prompt.includes("x".repeat(24_001)));
  assert.ok(prompt.endsWith("[truncated]"));
});

test("parseSseLine extracts delta content from data lines", () => {
  assert.deepEqual(
    parseSseLine('data: {"choices":[{"delta":{"content":"Hello"}}]}'),
    { type: "delta", content: "Hello" },
  );
});

test("parseSseLine reports the [DONE] sentinel", () => {
  assert.deepEqual(parseSseLine("data: [DONE]"), { type: "done" });
});

test("parseSseLine ignores comments, empty, and malformed lines", () => {
  assert.deepEqual(parseSseLine(": keep-alive"), { type: "none" });
  assert.deepEqual(parseSseLine(""), { type: "none" });
  assert.deepEqual(parseSseLine("event: message"), { type: "none" });
  assert.deepEqual(parseSseLine("data: {not json"), { type: "none" });
  assert.deepEqual(parseSseLine('data: {"choices":[{"delta":{}}]}'), {
    type: "none",
  });
});

test("feedSseChunk buffers deltas split across chunks", () => {
  const first = feedSseChunk(
    "",
    'data: {"choices":[{"delta":{"content":"Hel"}}]}\n' +
      'data: {"choices":[{"delta":{"content":"lo"}}]}\n' +
      'data: {"choices":[{"delta":{"cont',
  );
  assert.deepEqual(first.deltas, ["Hel", "lo"]);
  assert.equal(first.done, false);

  const second = feedSseChunk(
    first.buffer,
    'ent":" world"}}]}\ndata: [DONE]\n',
  );
  assert.deepEqual(second.deltas, [" world"]);
  assert.equal(second.done, true);
  assert.equal(second.buffer, "");
});

test("SharedNoteChatError surfaces the response status", () => {
  const error = new SharedNoteChatError(429);

  assert.ok(error instanceof Error);
  assert.equal(error.name, "SharedNoteChatError");
  assert.equal(error.status, 429);
});

test("chat history retains only the newest bounded context", () => {
  const messages = Array.from({ length: 100 }, (_, index) => ({
    role: index % 2 === 0 ? ("user" as const) : ("assistant" as const),
    content: `${index}:`.padEnd(2_000, "x"),
  }));

  const bounded = boundSharedNoteChatMessages(messages);
  const chars = bounded.reduce(
    (total, message) => total + message.content.length,
    0,
  );

  assert.ok(bounded.length <= MAX_SHARED_NOTE_CHAT_MESSAGES);
  assert.ok(chars <= MAX_SHARED_NOTE_CHAT_CONTEXT_CHARS);
  assert.ok(bounded.at(-1)?.content.startsWith("99:"));
  assert.ok(!bounded.some((message) => message.content.startsWith("0:")));
});

test("appending messages and streamed output stays bounded", () => {
  let messages: SharedNoteChatMessage[] = [];
  for (let index = 0; index < 100; index += 1) {
    messages = appendSharedNoteChatMessage(messages, {
      role: "user",
      content: `${index}`.repeat(1_000),
    });
  }
  assert.ok(messages.length <= MAX_SHARED_NOTE_CHAT_MESSAGES);

  const response = appendSharedNoteChatResponse(
    "x".repeat(MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS - 1),
    "y".repeat(1_000),
  );
  assert.equal(response.length, MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS);
});

test("settled assistant replies retain the full streamed response", () => {
  const response = "x".repeat(MAX_SHARED_NOTE_CHAT_RESPONSE_CHARS);

  const messages = appendSharedNoteChatMessage([], {
    role: "assistant",
    content: response,
  });

  assert.equal(messages[0]?.content, response);
  assert.equal(boundSharedNoteChatMessages(messages)[0]?.content, response);
});

test("unterminated SSE lines cannot grow the carry buffer indefinitely", () => {
  const result = feedSseChunk(
    "",
    "x".repeat(MAX_SHARED_NOTE_SSE_BUFFER_CHARS * 2),
  );

  assert.equal(result.buffer.length, MAX_SHARED_NOTE_SSE_BUFFER_CHARS);
});
