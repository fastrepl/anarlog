import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";

import {
  SESSION_SPEAKERS_SQL,
  SESSION_TRANSCRIPTS_SQL,
  applyTranscriptEdits,
  sharedMicrophoneContextStatement,
  transcriptEditPatch,
  transcriptEditStatements,
} from "./transcript-model.ts";

test("transcript edits preserve the recognized text and append revision history", () => {
  const source = [
    {
      id: "transcript:segment-1",
      text: "deploy to stagin",
      speaker: "Speaker 1",
      startMs: 100,
      endMs: 900,
      wordCount: 3,
    },
  ];
  const firstPatch = transcriptEditPatch(
    source[0].id,
    source[0].text,
    "deploy to staging",
    "2026-11-03T00:00:00.000Z",
  );
  const first = applyTranscriptEdits(source, JSON.stringify(firstPatch));
  assert.equal(first[0].text, "deploy to staging");
  assert.equal(first[0].originalText, source[0].text);
  assert.equal(first[0].revisionCount, 1);
  const secondPatch = transcriptEditPatch(
    source[0].id,
    source[0].text,
    "deploy to staging today",
    "2026-11-03T00:01:00.000Z",
    firstPatch.transcript_edits[source[0].id],
  );
  assert.deepEqual(secondPatch.transcript_edits[source[0].id].revisions, [
    { text: source[0].text, edited_at: "2026-11-03T00:00:00.000Z" },
    { text: "deploy to staging", edited_at: "2026-11-03T00:01:00.000Z" },
  ]);
  assert.equal(source[0].text, "deploy to stagin");
});

test("malformed transcript edit metadata leaves recognized words untouched", () => {
  const source = [
    {
      id: "transcript:segment-1",
      text: "recognized",
      speaker: "Speaker 1",
      startMs: 0,
      endMs: 100,
      wordCount: 1,
    },
  ];
  assert.equal(applyTranscriptEdits(source, "not json"), source);
  assert.equal(
    applyTranscriptEdits(source, JSON.stringify({ transcript_edits: [] })),
    source,
  );
});

test("saving an edit preserves raw words, records history, and invalidates minutes atomically", () => {
  const db = new DatabaseSync(":memory:");
  try {
    db.exec(`CREATE TABLE transcripts (
      id TEXT, session_id TEXT, words_json TEXT, metadata_json TEXT,
      updated_at TEXT, deleted_at TEXT
    );
    CREATE TABLE session_documents (
      id TEXT, session_id TEXT, kind TEXT, generation_metadata_json TEXT,
      updated_at TEXT, deleted_at TEXT
    );
    INSERT INTO transcripts VALUES ('transcript', 'session', '[{"text":"stagin"}]', '{}', '', NULL);
    INSERT INTO session_documents VALUES ('summary', 'session', 'summary', '{"model":"test"}', '', NULL);`);
    const patch = transcriptEditPatch(
      "transcript:segment-1",
      "deploy to stagin",
      "deploy to staging",
      "2026-11-03T00:00:00.000Z",
    );
    const statements = transcriptEditStatements(
      "transcript",
      "session",
      "{}",
      JSON.stringify(patch),
      "2026-11-03T00:00:00.000Z",
    );
    const [transcriptUpdate, summaryUpdate] = statements;
    db.prepare(transcriptUpdate.sql).run(...transcriptUpdate.params);
    db.prepare(summaryUpdate.sql).run(...summaryUpdate.params);
    const saved = db.prepare("SELECT * FROM transcripts").get();
    assert.equal(saved.words_json, '[{"text":"stagin"}]');
    assert.equal(
      JSON.parse(saved.metadata_json).transcript_edits["transcript:segment-1"]
        .text,
      "deploy to staging",
    );
    const summary = db.prepare("SELECT * FROM session_documents").get();
    assert.deepEqual(JSON.parse(summary.generation_metadata_json), {
      model: "test",
      transcript_stale: true,
    });

    const conflict = transcriptEditStatements(
      "transcript",
      "session",
      "outdated metadata", // Optimistic guard refuses to overwrite another edit.
      JSON.stringify(patch),
      "2026-11-03T00:02:00.000Z",
    );
    db.prepare(
      'UPDATE session_documents SET generation_metadata_json = \'{"model":"test","transcript_stale":false}\'',
    ).run();
    db.prepare(conflict[0].sql).run(...conflict[0].params);
    db.prepare(conflict[1].sql).run(...conflict[1].params);
    assert.equal(
      JSON.parse(
        db
          .prepare("SELECT generation_metadata_json FROM session_documents")
          .get().generation_metadata_json,
      ).transcript_stale,
      false,
    );
  } finally {
    db.close();
  }
});

test("historical speaker labels survive contact deletion within their workspace", () => {
  const db = new DatabaseSync(":memory:");
  try {
    db.exec(`CREATE TABLE sessions (id TEXT, workspace_id TEXT);
      CREATE TABLE humans (id TEXT, workspace_id TEXT, name TEXT, deleted_at TEXT);
      INSERT INTO sessions VALUES ('session', 'workspace');
      INSERT INTO humans VALUES ('artem', 'workspace', 'Artem', '2026-09-24'),
        ('other', 'other-workspace', 'Other', NULL);`);
    const names = new Map(
      db
        .prepare(SESSION_SPEAKERS_SQL)
        .all("session")
        .map(({ id, name }) => [id, name]),
    );
    assert.deepEqual([...names], [["artem", "Artem"]]);
  } finally {
    db.close();
  }
});

test("mobile capture context survives reload without overwriting desktop intervals", () => {
  const db = new DatabaseSync(":memory:");
  try {
    const desktop = { start_ms: 1, end_ms: 100, shared_microphone: false };
    db.exec(`CREATE TABLE sessions (id TEXT, metadata_json TEXT, updated_at TEXT, deleted_at TEXT);
      CREATE TABLE transcripts (source TEXT, provider TEXT, id TEXT, session_id TEXT, started_at_ms INTEGER, owner_user_id TEXT,
        words_json TEXT, speaker_hints_json TEXT, metadata_json TEXT DEFAULT '{}', deleted_at TEXT, created_at TEXT);
      CREATE TABLE transcript_live_deltas (transcript_id TEXT, sequence INTEGER, delta_json TEXT);
      INSERT INTO transcripts (id, session_id, started_at_ms, owner_user_id, words_json, speaker_hints_json, deleted_at, created_at) VALUES ('transcript', 'session', 1000, 'john', '[]', '[]', NULL, 'now');`);
    db.prepare("INSERT INTO sessions VALUES (?, ?, '', NULL)").run(
      "session",
      JSON.stringify({
        other: "retained",
        speaker_context: { intervals: [desktop] },
      }),
    );
    const statement = sharedMicrophoneContextStatement(
      "session",
      1000,
      2500,
      "now",
    );
    db.prepare(statement.sql).run(...statement.params);
    const row = db.prepare(SESSION_TRANSCRIPTS_SQL).get("session");
    assert.equal(row.owner_user_id, "john");
    const context = JSON.parse(row.speaker_context_json);
    assert.deepEqual(context.intervals[0], desktop);
    assert.deepEqual(context.intervals[1], {
      start_ms: 1000,
      end_ms: 3500,
      shared_microphone: true,
      mic_isolated: null,
      active_call: false,
      calendar_call: false,
      title: "",
      self_names: [],
      participants: [],
    });
    assert.equal(
      JSON.parse(
        db.prepare("SELECT metadata_json FROM sessions").get().metadata_json,
      ).other,
      "retained",
    );
  } finally {
    db.close();
  }
});
