import { cleanup, render, screen } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PromptEditor, promptDocToText, promptTextToDoc } from ".";

describe("prompt editor serialization", () => {
  afterEach(cleanup);

  it("turns supported template expressions into atomic token nodes", () => {
    const doc = promptTextToDoc("Before {{template}} after");
    const paragraph = doc.firstChild;

    expect(paragraph?.childCount).toBe(3);
    expect(paragraph?.child(1).type.name).toBe("promptToken");
    expect(paragraph?.child(1).attrs.name).toBe("template");
    expect(promptDocToText(doc)).toBe("Before {{ template }} after");
  });

  it("preserves paragraphs, empty lines, and unknown expressions", () => {
    const value = "First line\n\nKeep {{ unknown }} as text\nLast line";
    expect(promptDocToText(promptTextToDoc(value))).toBe(value);
  });

  it("normalizes Windows line endings", () => {
    expect(promptDocToText(promptTextToDoc("First\r\nSecond"))).toBe(
      "First\nSecond",
    );
  });

  it("renders template expressions as visible atomic chips", () => {
    const { container } = render(
      createElement(PromptEditor, {
        ariaLabel: "Summary instructions",
        initialValue: "Follow {{ template }}",
        onChange: vi.fn(),
      }),
    );

    expect(
      screen.getByRole("textbox", { name: "Summary instructions" }),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-prompt-token="template"]')?.textContent,
    ).toBe("Template");
  });
});
