import { afterEach, describe, expect, it } from "vitest";

import {
  getTranscriptContextSelection,
  getTranscriptSelectionFromRange,
  mergeTranscriptSelections,
} from "./selection";

afterEach(() => {
  document.body.replaceChildren();
});

describe("transcript word selection", () => {
  it("maps a native text range to stable word ids", () => {
    const { container, words } = createReadSegment();
    const range = document.createRange();
    range.setStart(words[0]!.firstChild!, 0);
    range.setEnd(words[1]!.firstChild!, words[1]!.textContent!.length);

    expect(getTranscriptSelectionFromRange(range, container)).toEqual({
      sessionId: "session-1",
      text: "One Two",
      startMs: 1100,
      groups: [
        {
          transcriptId: "transcript-1",
          segmentKey: {
            channel: "RemoteParty",
            speaker_index: 1,
            speaker_human_id: null,
          },
          wordIds: ["word-1", "word-2"],
        },
      ],
    });
  });

  it("uses the containing entry when opening a context menu without a range", () => {
    const { container, words } = createReadSegment();

    expect(
      getTranscriptContextSelection({
        target: words[1],
        container,
      })?.selection.groups[0]?.wordIds,
    ).toEqual(["word-1", "word-2", "word-3"]);
  });

  it("maps write-mode character selection back to transcript words", () => {
    const container = document.createElement("div");
    const section = createSection();
    const editor = document.createElement("div");
    editor.dataset.transcriptSegmentContent = "";
    editor.dataset.transcriptEditor = "";
    editor.dataset.transcriptEditWordIds = JSON.stringify([
      "word-1",
      "word-2",
      "word-3",
    ]);
    editor.dataset.transcriptEditWordTexts = JSON.stringify([
      "One",
      "Two",
      "Three",
    ]);
    editor.dataset.transcriptEditWordStartMs = JSON.stringify([100, 200, 300]);
    editor.textContent = "One Two Three";
    section.append(editor);
    container.append(section);
    document.body.append(container);
    const range = document.createRange();
    range.setStart(editor.firstChild!, 4);
    range.setEnd(editor.firstChild!, 7);

    expect(
      getTranscriptSelectionFromRange(range, container)?.groups[0]?.wordIds,
    ).toEqual(["word-2"]);
  });

  it("combines scattered entry selections by transcript", () => {
    expect(
      mergeTranscriptSelections([
        {
          sessionId: "session-1",
          text: "One",
          startMs: 100,
          groups: [
            {
              transcriptId: "transcript-1",
              segmentKey: {
                channel: "RemoteParty",
                speaker_index: 1,
                speaker_human_id: null,
              },
              wordIds: ["word-1"],
            },
          ],
        },
        {
          sessionId: "session-1",
          text: "Three",
          startMs: 300,
          groups: [
            {
              transcriptId: "transcript-1",
              segmentKey: {
                channel: "RemoteParty",
                speaker_index: 2,
                speaker_human_id: null,
              },
              wordIds: ["word-3"],
            },
          ],
        },
      ])?.groups[0]?.wordIds,
    ).toEqual(["word-1", "word-3"]);
  });
});

function createReadSegment() {
  const container = document.createElement("div");
  const section = createSection();
  const content = document.createElement("div");
  content.dataset.transcriptSegmentContent = "";
  const words = ["One", "Two", "Three"].map((text, index) => {
    const word = document.createElement("span");
    word.dataset.transcriptWordId = `word-${index + 1}`;
    word.dataset.transcriptWordStartMs = String((index + 1) * 100);
    word.textContent = text;
    content.append(word);
    if (index < 2) content.append(document.createTextNode(" "));
    return word;
  });
  section.append(content);
  container.append(section);
  document.body.append(container);
  return { container, words };
}

function createSection() {
  const section = document.createElement("section");
  section.dataset.transcriptId = "transcript-1";
  section.dataset.sessionId = "session-1";
  section.dataset.segmentChannel = "RemoteParty";
  section.dataset.segmentSpeakerIndex = "1";
  section.dataset.segmentSpeakerHumanId = "";
  section.dataset.transcriptOffsetMs = "1000";
  return section;
}
