import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TemplateIconPicker } from "./template-icon-picker";

afterEach(cleanup);

describe("TemplateIconPicker", () => {
  it("selects a colored icon", () => {
    const onChange = vi.fn();
    render(
      <TemplateIconPicker
        value={{ type: "icon", value: "notebook-tabs", color: "#9ca3af" }}
        onChange={onChange}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Choose template icon" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Use #5b67d8" }));
    fireEvent.click(screen.getByRole("button", { name: "target" }));

    expect(onChange).toHaveBeenLastCalledWith({
      type: "icon",
      value: "target",
      color: "#5b67d8",
    });
  });

  it("searches and selects an emoji", () => {
    const onChange = vi.fn();
    render(
      <TemplateIconPicker
        value={{ type: "icon", value: "notebook-tabs", color: "#9ca3af" }}
        onChange={onChange}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Choose template icon" }),
    );
    fireEvent.click(screen.getByRole("tab", { name: "Emojis" }));
    fireEvent.change(screen.getByPlaceholderText("Search emoji..."), {
      target: { value: "rocket" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Rocket" }));

    expect(onChange).toHaveBeenLastCalledWith({
      type: "emoji",
      value: "🚀",
    });
  });
});
