import assert from "node:assert/strict";
import { test } from "node:test";

import { createNoteDraft } from "./note-draft.ts";

const note = {
  id: "session",
  title: "Original",
  createdAt: "",
  noteText: "",
  bodyFormat: "prosemirror_json",
  plainEditable: true,
  summary: null,
};

function setup(overrides = {}) {
  const writes = [];
  const draft = createNoteDraft({
    getNote: () => note,
    saveNote: async (value) => {
      writes.push(value);
    },
    saveTitle: async (title) => {
      writes.push({ title });
    },
    onError: () => {},
    ...overrides,
  });
  return { draft, writes };
}

test("body autosaves retain the edited title until the live query catches up", async () => {
  let current = note;
  const { draft, writes } = setup({ getNote: () => current });
  draft.edit({ title: "Renamed" });
  await draft.flush();
  draft.edit({ body: "New body", bodyFormat: "markdown" });
  await draft.flush();
  current = { ...note, title: "Renamed" };
  draft.observeTitle(current.title);
  current = { ...note, title: "Title from sync" };
  draft.edit({ body: "After sync" });
  await draft.flush();
  assert.deepEqual(writes, [
    { title: "Renamed" },
    { title: "Renamed", bodyText: "New body", bodyFormat: "markdown" },
    {
      title: "Title from sync",
      bodyText: "After sync",
      bodyFormat: "prosemirror_json",
    },
  ]);
});

test("debouncing coalesces typing and discard prevents a deleted note from being saved", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { draft, writes } = setup();
  draft.edit({ title: "First" });
  t.mock.timers.tick(400);
  draft.edit({ title: "Latest", body: "Body" });
  t.mock.timers.tick(499);
  assert.deepEqual(writes, []);
  t.mock.timers.tick(1);
  await Promise.resolve();
  assert.deepEqual(writes, [
    { title: "Latest", bodyText: "Body", bodyFormat: "prosemirror_json" },
  ]);
  draft.edit({ body: "Discarded" });
  draft.discard();
  t.mock.timers.tick(500);
  await draft.flush();
  assert.equal(writes.length, 1);
});

test("a failed save keeps newer typing for retry and can block summary generation", async () => {
  const failure = new Error("Disk full");
  let rejectSave;
  let attempt = 0;
  const { draft, writes } = setup({
    saveNote: (value) => {
      if (attempt++ === 0)
        return new Promise((_, reject) => {
          rejectSave = reject;
        });
      writes.push(value);
      return Promise.resolve();
    },
  });
  draft.edit({ title: "New title", body: "First body" });
  const saving = draft.flush(true);
  draft.edit({ body: "Latest body", bodyFormat: "markdown" });
  rejectSave(failure);
  await assert.rejects(saving, failure);
  await draft.flush();
  assert.deepEqual(writes, [
    { title: "New title", bodyText: "Latest body", bodyFormat: "markdown" },
  ]);
});

test("restoring a title discards stale edits but preserves that title in subsequent body saves", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const { draft, writes } = setup();
  draft.edit({ title: "Stale title", body: "Stale body" });
  draft.restore("Restored title");
  t.mock.timers.tick(500);
  assert.deepEqual(writes, []);
  draft.edit({ body: "After restore" });
  await draft.flush();
  assert.deepEqual(writes, [
    {
      title: "Restored title",
      bodyText: "After restore",
      bodyFormat: "prosemirror_json",
    },
  ]);
});

test("version restore waits for an autosave and late failure cannot revive discarded edits", async () => {
  let finish;
  let persisted = "Original";
  const { draft } = setup({
    saveNote: (value) =>
      new Promise((resolve) => {
        finish = () => {
          persisted = value.bodyText;
          resolve();
        };
      }),
  });
  draft.edit({ body: "New edit" });
  const saving = draft.flush();
  const restoring = (async () => {
    await draft.flush(true);
    persisted = "Previous version";
    draft.restore("Restored title");
  })();
  await Promise.resolve();
  assert.equal(persisted, "Original");
  finish();
  await Promise.all([saving, restoring]);
  assert.equal(persisted, "Previous version");
  assert.deepEqual(draft.snapshot(), {});

  let reject;
  const failed = setup({
    saveNote: () =>
      new Promise((_, fail) => {
        reject = fail;
      }),
  }).draft;
  failed.edit({ body: "Stale" });
  const attempt = failed.flush();
  failed.restore("Restored");
  reject(new Error("Disk full"));
  await attempt;
  assert.deepEqual(failed.snapshot(), {});
});

test("failed deletion automatically retries edits from a failed autosave", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let rejectSave;
  let attempt = 0;
  const { draft, writes } = setup({
    saveNote: (value) => {
      if (attempt++ === 0)
        return new Promise((_, reject) => {
          rejectSave = reject;
        });
      writes.push(value);
      return Promise.resolve();
    },
  });
  draft.edit({ body: "Unsaved edits" });
  const saving = draft.flush();
  const deleting = draft.remove(async () => {
    throw new Error("Deletion failed");
  });
  const rejected = assert.rejects(deleting, /Deletion failed/);
  rejectSave(new Error("Save failed"));
  await Promise.all([saving, rejected]);
  t.mock.timers.tick(500);
  await Promise.resolve();
  assert.deepEqual(writes, [
    {
      title: "Original",
      bodyText: "Unsaved edits",
      bodyFormat: "prosemirror_json",
    },
  ]);
});
