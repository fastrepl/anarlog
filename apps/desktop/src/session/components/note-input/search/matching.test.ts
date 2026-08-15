import { describe, expect, it, vi } from "vitest";

import {
  getMatchingElements,
  getTranscriptMatches,
  getTranscriptWordMatches,
  registerTranscriptSearchSource,
  type SearchOptions,
} from "./matching";

const defaultOptions: SearchOptions = {
  caseSensitive: false,
  wholeWord: false,
};

describe("getTranscriptMatches", () => {
  it("returns transcript word spans in match order", () => {
    const spans = [
      createSpan("word-1", "Hello"),
      createSpan("word-2", "world"),
      createSpan("word-3", "hello"),
    ];

    expect(
      getTranscriptMatches(spans, "hello", defaultOptions).map(
        (match) => match.id,
      ),
    ).toEqual(["word-1", "word-3"]);
  });

  it("maps phrase matches to the first matching word", () => {
    const spans = [
      createSpan("word-1", "plan"),
      createSpan("word-2", "the"),
      createSpan("word-3", "launch"),
    ];

    expect(
      getTranscriptMatches(spans, "the launch", defaultOptions).map(
        (match) => match.id,
      ),
    ).toEqual(["word-2"]);
  });

  it("keeps whole-word matching behavior", () => {
    const spans = [
      createSpan("word-1", "sync"),
      createSpan("word-2", "async"),
      createSpan("word-3", "syncing"),
    ];

    expect(
      getTranscriptMatches(spans, "sync", {
        ...defaultOptions,
        wholeWord: true,
      }).map((match) => match.id),
    ).toEqual(["word-1"]);
  });

  it("searches registered virtual transcript words and navigates off-screen", () => {
    const container = document.createElement("div");
    const scrollIntoView = vi.fn();
    const unregister = registerTranscriptSearchSource(
      container,
      (prepared, options) =>
        getTranscriptWordMatches(
          [
            { id: "word-1", text: "visible", scrollIntoView: vi.fn() },
            { id: "word-999", text: "offscreen target", scrollIntoView },
          ],
          prepared,
          options,
        ),
    );

    const matches = getMatchingElements(container, "target", defaultOptions);
    expect(matches.map((match) => match.id)).toEqual(["word-999"]);
    matches[0].element.scrollIntoView();
    expect(scrollIntoView).toHaveBeenCalledOnce();

    unregister();
    expect(getMatchingElements(container, "target", defaultOptions)).toEqual(
      [],
    );
  });
});

function createSpan(id: string, text: string) {
  const span = document.createElement("span");
  span.dataset.wordId = id;
  span.textContent = text;
  return span;
}
