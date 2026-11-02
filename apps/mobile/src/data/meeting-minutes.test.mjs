import assert from "node:assert/strict";
import test from "node:test";

import {
  cloudflareMeetingMinutesRequest,
  readCloudflareMeetingMinutes,
  renderMeetingMinutes,
} from "./meeting-minutes.ts";

const segment = {
  id: "transcript-a:segment-1",
  speaker: "Speaker 1",
  text: "Nam will review the staging deployment by Friday.",
  start_ms: 1_000,
  end_ms: 2_000,
};
const input = {
  model: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
  meeting_date: null,
  timezone: "Asia/Ho_Chi_Minh",
  language: "Vietnamese",
  transcript_revision: "revision-1",
  notes: "",
  segments: [segment],
};

function response(evidenceId = segment.id) {
  return {
    minutes: {
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
          evidence_segment_ids: [evidenceId],
          evidence_quotes: [
            {
              segment_id: evidenceId,
              quote: "Nam will review the staging deployment",
            },
          ],
        },
      ],
      open_questions: [],
      suggestions: [{ id: "suggestion-1", text: "Add a rollback checklist." }],
    },
    metadata: {
      model: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
      prompt_version: "3",
      schema_version: "2",
      transcript_revision: "revision-1",
      input_tokens: 400,
      output_tokens: 120,
      total_tokens: 520,
      duration_ms: 3_200,
    },
  };
}

test("Worker request sends transcript text and session token to the configured URL", () => {
  const request = cloudflareMeetingMinutesRequest(
    "https://minutes.example/",
    "session-token",
    input,
  );
  assert.equal(request.url, "https://minutes.example/v1/meeting-minutes");
  assert.equal(request.headers.Authorization, "Bearer session-token");
  assert.deepEqual(JSON.parse(request.body).segments, [segment]);
  assert.equal("audio" in JSON.parse(request.body), false);
});

test("client rejects evidence references outside the request revision", () => {
  assert.throws(() =>
    readCloudflareMeetingMinutes(response("another-revision:segment"), [
      segment,
    ]),
  );
});

test("structured minutes preserve review state and separate AI suggestions", () => {
  const minutes = readCloudflareMeetingMinutes(response(), [segment]);
  const markdown = renderMeetingMinutes(minutes, "Vietnamese");
  assert.match(markdown, /Cần xác nhận/u);
  assert.match(markdown, /Dẫn chứng/u);
  assert.match(markdown, /Nam will review the staging deployment/u);
  assert.match(
    markdown,
    /\[transcript-a:segment-1\]\(anarlog-evidence:\/\/transcript-a%3Asegment-1\)/u,
  );
  assert.match(markdown, /Gợi ý của AI/u);
  assert.equal(minutes.metadata.total_tokens, 520);
});
