import assert from "node:assert/strict";
import test from "node:test";

import {
  createSharedNoteParticipantPresentation,
  findFeaturedSharedNoteAudio,
  isSharedNoteAudioGrantExpiring,
} from "./shared-note-presentation.ts";

test("builds the shared meeting participant row", () => {
  assert.deepEqual(
    createSharedNoteParticipantPresentation([
      " John  Jeong ",
      "John Jeong",
      "Artem",
      "Sungbin Jo",
      "Yujong Lee",
      "Julie",
      "Charlie",
    ]),
    {
      avatarParticipants: [
        "John Jeong",
        "Artem",
        "Sungbin Jo",
        "Yujong Lee",
        "Julie",
      ],
      label: "John, Artem +4 more",
      participantCount: 6,
    },
  );
});

test("finds the first playable shared audio attachment", () => {
  const audio = {
    id: "audio",
    filename: "meeting.m4a",
    contentType: "audio/mp4",
    sizeBytes: 10,
    sha256: "a".repeat(64),
  };
  assert.equal(
    findFeaturedSharedNoteAudio([
      { ...audio, id: "image", contentType: "image/png" },
      audio,
      { ...audio, id: "second" },
    ]),
    audio,
  );
});

test("refreshes audio grants before they expire", () => {
  const now = Date.parse("2026-07-23T12:00:00Z");
  assert.equal(
    isSharedNoteAudioGrantExpiring("2026-07-23T12:00:10Z", now),
    true,
  );
  assert.equal(
    isSharedNoteAudioGrantExpiring("2026-07-23T12:00:11Z", now),
    false,
  );
});
