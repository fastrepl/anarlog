import assert from "node:assert/strict";
import test from "node:test";

import {
  MeetingMinutesSchema,
  MeetingMinutesRequestSchema,
  validateEvidenceReferences,
} from "./index.ts";

const segment = {
  id: "transcript-a:segment-1",
  speaker: "Speaker 1",
  text: "Nam will review the staging deployment by Friday.",
  start_ms: 1_000,
  end_ms: 2_000,
};
const evidenceQuotes = [
  { segment_id: segment.id, quote: "Nam will review the staging deployment" },
];

test("meeting input rejects duplicate evidence segment IDs", () => {
  const parsed = MeetingMinutesRequestSchema.safeParse({
    model: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
    meeting_date: null,
    timezone: "Asia/Ho_Chi_Minh",
    language: "Vietnamese",
    transcript_revision: "revision-1",
    notes: "",
    segments: [segment, segment],
  });
  assert.equal(parsed.success, false);
});

test("meeting input rejects model IDs outside the supported model allowlist", () => {
  const parsed = MeetingMinutesRequestSchema.safeParse({
    model: "@cf/qwen/qwen4-not-a-real-model",
    meeting_date: null,
    timezone: "Asia/Ho_Chi_Minh",
    language: "Vietnamese",
    transcript_revision: "revision-1",
    notes: "",
    segments: [segment],
  });
  assert.equal(parsed.success, false);
});

test("meeting input allows the Qwen benchmark candidates without JSON Mode", () => {
  for (const model of ["@cf/qwen/qwen3-30b-a3b-fp8", "@cf/qwen/qwen3.8-27b"]) {
    const parsed = MeetingMinutesRequestSchema.safeParse({
      model,
      meeting_date: null,
      timezone: "Asia/Ho_Chi_Minh",
      language: "Vietnamese",
      transcript_revision: "revision-1",
      notes: "",
      segments: [segment],
    });
    assert.equal(parsed.success, true);
  }
});

test("action items keep ambiguous ownership and dates reviewable", () => {
  const minutes = MeetingMinutesSchema.parse({
    title: "Staging review",
    summary: "The team discussed staging.",
    decisions: [],
    action_items: [
      {
        id: "action-1",
        task: "Review the deployment",
        owner_id: null,
        owner_text: "Nam",
        deadline_text: "by Friday",
        deadline_iso: null,
        status: "needs_confirmation",
        evidence_segment_ids: [segment.id],
        evidence_quotes: evidenceQuotes,
      },
    ],
    open_questions: [],
    suggestions: [],
  });
  assert.equal(minutes.action_items[0].deadline_iso, null);
  assert.equal(minutes.action_items[0].status, "needs_confirmation");
  assert.doesNotThrow(() => validateEvidenceReferences(minutes, [segment]));
});

test("evidence validation rejects IDs outside the transcript revision", () => {
  const minutes = MeetingMinutesSchema.parse({
    title: "Staging review",
    summary: "The team discussed staging.",
    decisions: [
      {
        id: "decision-1",
        text: "Use staging for validation.",
        evidence_segment_ids: ["foreign-segment"],
        evidence_quotes: [
          { segment_id: "foreign-segment", quote: "Use staging" },
        ],
      },
    ],
    action_items: [],
    open_questions: [],
    suggestions: [],
  });
  assert.throws(() => validateEvidenceReferences(minutes, [segment]));
});

test("evidence validation rejects a fabricated quote even when its segment ID is valid", () => {
  const minutes = MeetingMinutesSchema.parse({
    title: "Staging review",
    summary: "The team discussed staging.",
    decisions: [
      {
        id: "decision-1",
        text: "The deployment was approved.",
        evidence_segment_ids: [segment.id],
        evidence_quotes: [
          { segment_id: segment.id, quote: "The deployment was approved." },
        ],
      },
    ],
    action_items: [],
    open_questions: [],
    suggestions: [],
  });
  assert.throws(() => validateEvidenceReferences(minutes, [segment]), /quote/u);
});
