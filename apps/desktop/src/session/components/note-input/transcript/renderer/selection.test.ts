import { afterEach, describe, expect, it } from "vitest";

import {
  canMergeTranscriptEntries,
  getTranscriptContextSelection,
  getTranscriptMergeTarget,
  getTranscriptRangeRects,
  getTranscriptSelectionFromRange,
  getTranscriptSelectionFromSegment,
  isRangeCoveredBySelection,
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

  it("builds a selection from segment data without reading the DOM", () => {
    expect(
      getTranscriptSelectionFromSegment({
        transcriptId: "transcript-1",
        sessionId: "session-1",
        offsetMs: 1000,
        segment: {
          key: {
            channel: "RemoteParty",
            speaker_index: 1,
            speaker_human_id: null,
          },
          text: "One Two",
          words: [
            {
              id: "word-1",
              text: "One",
              start_ms: 100,
              end_ms: 160,
              channel: "RemoteParty",
              is_final: true,
            },
            {
              id: "word-2",
              text: "Two",
              start_ms: 180,
              end_ms: 240,
              channel: "RemoteParty",
              is_final: true,
            },
          ],
        },
      }),
    ).toEqual({
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

  it("allows merging only contiguous same-channel transcript entries", () => {
    const order = ["a", "b", "c"];
    const entries = new Map([
      ["a", entry("transcript-1", 0, "alice", "word-1")],
      ["b", entry("transcript-1", 1, null, "word-2")],
      ["c", entry("transcript-1", 2, null, "word-3")],
    ]);

    expect(canMergeTranscriptEntries(new Set(["a", "c"]), order, entries)).toBe(
      false,
    );
    expect(canMergeTranscriptEntries(new Set(["a", "b"]), order, entries)).toBe(
      true,
    );
    expect(
      canMergeTranscriptEntries(
        new Set(["a", "b"]),
        order,
        new Map([
          ["a", entry("transcript-1", 0, null, "word-1")],
          ["b", entry("transcript-1", 1, null, "word-2")],
        ]),
      ),
    ).toBe(true);
    expect(
      canMergeTranscriptEntries(
        new Set(["a", "b"]),
        order,
        new Map([
          ["a", entry("transcript-1", null, null, "word-1")],
          ["b", entry("transcript-1", 1, null, "word-2")],
        ]),
      ),
    ).toBe(false);
    expect(
      getTranscriptMergeTarget(new Set(["a", "b"]), order, entries)?.groups[0],
    ).toEqual({
      transcriptId: "transcript-1",
      segmentKey: {
        channel: "RemoteParty",
        speaker_index: 0,
        speaker_human_id: "alice",
      },
      wordIds: ["word-1"],
    });
    expect(
      canMergeTranscriptEntries(
        new Set(["a", "b"]),
        order,
        new Map([
          ["a", entry("transcript-1", 0, null, "word-1")],
          ["b", entry("transcript-2", 1, null, "word-2")],
        ]),
      ),
    ).toBe(false);
  });
});

function entry(
  transcriptId: string,
  speakerIndex: number | null,
  speakerHumanId: string | null,
  wordId: string,
) {
  return {
    text: wordId,
    startMs: 0,
    groups: [
      {
        transcriptId,
        segmentKey: {
          channel: "RemoteParty" as const,
          speaker_index: speakerIndex,
          speaker_human_id: speakerHumanId,
        },
        wordIds: [wordId],
      },
    ],
  };
}

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

describe("transcript selection overlay", () => {
  afterEach(() => {
    Reflect.deleteProperty(Range.prototype, "getClientRects");
    window.getSelection()?.removeAllRanges();
  });

  it("collects text rects per line instead of element boxes", () => {
    const container = document.createElement("div");
    container.innerHTML =
      '<span data-line="0"><span>One</span> <span>two</span></span><span data-line="1"><span>three</span></span>';
    document.body.append(container);
    const offsets = new Map<Node, number>();
    const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
    let offset = 0;
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      offsets.set(node, offset);
      offset += node.textContent?.length ?? 0;
    }
    Object.assign(Range.prototype, {
      getClientRects(this: Range) {
        const text = this.toString();
        if (!text) {
          return [];
        }
        const line = Number(
          this.startContainer.parentElement
            ?.closest("[data-line]")
            ?.getAttribute("data-line") ?? 0,
        );
        const left =
          ((offsets.get(this.startContainer) ?? 0) + this.startOffset) * 10;
        return [new DOMRect(left, line * 20, text.length * 10, 20)];
      },
    });
    const textNodes = [...offsets.keys()];

    const range = document.createRange();
    range.setStart(textNodes[0]!, 0);
    range.setEnd(textNodes[3]!, 5);
    expect(getTranscriptRangeRects(range).map(toPlainRect)).toEqual([
      { left: 0, top: 0, width: 70, height: 20 },
      { left: 70, top: 20, width: 50, height: 20 },
    ]);

    range.setStart(textNodes[2]!, 1);
    range.setEnd(textNodes[3]!, 3);
    expect(getTranscriptRangeRects(range).map(toPlainRect)).toEqual([
      { left: 50, top: 0, width: 20, height: 20 },
      { left: 70, top: 20, width: 30, height: 20 },
    ]);
  });

  it("treats the range as covered while the native selection spans it", () => {
    const container = document.createElement("div");
    container.textContent = "One two three";
    document.body.append(container);
    const text = container.firstChild!;
    const range = document.createRange();
    range.setStart(text, 4);
    range.setEnd(text, 7);
    const selection = window.getSelection()!;

    expect(isRangeCoveredBySelection(range, selection)).toBe(false);

    const wider = document.createRange();
    wider.setStart(text, 0);
    wider.setEnd(text, 13);
    selection.addRange(wider);
    expect(isRangeCoveredBySelection(range, selection)).toBe(true);

    const narrower = document.createRange();
    narrower.setStart(text, 5);
    narrower.setEnd(text, 7);
    selection.removeAllRanges();
    selection.addRange(narrower);
    expect(isRangeCoveredBySelection(range, selection)).toBe(false);
  });
});

function toPlainRect({ left, top, width, height }: DOMRect) {
  return { left, top, width, height };
}
