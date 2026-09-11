import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";

import { MarkdownExportOptionsConfig } from "./markdown-export-options";

import { DEFAULT_MARKDOWN_EXPORT_OPTIONS } from "~/automations/markdown-export";

function Editor() {
  const [options, setOptions] = useState(DEFAULT_MARKDOWN_EXPORT_OPTIONS);
  return (
    <MarkdownExportOptionsConfig options={options} onChange={setOptions} />
  );
}

afterEach(cleanup);

describe("Markdown export options", () => {
  it("edits content, filenames and suffix independently", () => {
    render(<Editor />);
    for (const label of [
      "Memo",
      "Summary",
      "Transcript",
      "Collect action items",
      "Include ID [a1b2c3d4]",
    ]) {
      expect(
        (screen.getByRole("checkbox", { name: label }) as HTMLInputElement)
          .checked,
      ).toBe(true);
    }
    fireEvent.click(screen.getByRole("checkbox", { name: "Transcript" }));
    fireEvent.click(
      screen.getByRole("checkbox", { name: "Include ID [a1b2c3d4]" }),
    );
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "{date} {title} recap" },
    });
    expect(
      (screen.getByRole("checkbox", { name: "Transcript" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
    expect(
      (screen.getByRole("checkbox", { name: "Summary" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (
        screen.getByRole("checkbox", {
          name: "Include ID [a1b2c3d4]",
        }) as HTMLInputElement
      ).checked,
    ).toBe(false);
    expect(
      (screen.getByRole("textbox", { name: "Name" }) as HTMLInputElement).value,
    ).toBe("{date} {title} recap");
  });

  it("explains why an empty selection needs setup", () => {
    render(<Editor />);
    for (const label of [
      "Memo",
      "Summary",
      "Transcript",
      "Collect action items",
    ]) {
      fireEvent.click(screen.getByRole("checkbox", { name: label }));
    }
    expect(screen.getByRole("alert").textContent).toBe("Needs setup");
    fireEvent.click(screen.getByRole("checkbox", { name: "Memo" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
