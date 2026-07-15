import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Toast } from "./component";

describe("Toast", () => {
  it("renders sidebar notifications as compact action pills", () => {
    const onAdd = vi.fn();
    const onHide = vi.fn();

    const { container } = render(
      <Toast
        toast={{
          id: "missing-llm",
          description: "Language model needed",
          dismissible: true,
          primaryAction: {
            label: "Add",
            onClick: onAdd,
          },
        }}
        onDismiss={onHide}
      />,
    );

    const pill = container.querySelector(".inline-flex");

    expect(pill?.className).toContain("rounded-full");
    expect(pill?.className).toContain("bg-card");
    const description = screen.getByText("Language model needed");
    expect(description.className).not.toContain("truncate");
    expect(description.className).toContain("whitespace-normal");
    expect(screen.getByRole("button", { name: "Add" }).className).toContain(
      "bg-foreground",
    );
    expect(screen.getByRole("button", { name: "Add" }).className).toContain(
      "text-background",
    );
    expect(screen.getByRole("button", { name: "Hide" }).className).toContain(
      "bg-muted",
    );

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.click(screen.getByRole("button", { name: "Hide" }));

    expect(onAdd).toHaveBeenCalledTimes(1);
    expect(onHide).toHaveBeenCalledTimes(1);
  });

  it("uses the destructive button color for error primary actions", () => {
    render(
      <Toast
        toast={{
          id: "transcription-unavailable",
          description: "Transcription unavailable",
          dismissible: true,
          variant: "error",
          primaryAction: {
            label: "Settings",
            onClick: vi.fn(),
          },
        }}
      />,
    );

    const action = screen.getByRole("button", { name: "Settings" });

    expect(action.className).toContain("bg-destructive");
    expect(action.className).toContain("text-destructive-foreground");
  });

  it("wraps long warning toast text", () => {
    render(
      <Toast
        toast={{
          id: "transcription-language-warning",
          description: "Model doesn't support all languages.",
          dismissible: false,
          variant: "warning",
          actions: [
            {
              label: "Dismiss",
              onClick: vi.fn(),
            },
          ],
        }}
      />,
    );

    const description = screen.getByText(
      "Model doesn't support all languages.",
    );

    expect(description.className).not.toContain("truncate");
    expect(description.className).toContain("whitespace-normal");
    expect(description.className).toContain("break-words");
    expect(
      screen.getByRole("button", { name: "Dismiss" }).parentElement?.className,
    ).toContain("pl-2");
    expect(
      screen.getByRole("button", { name: "Dismiss" }).parentElement?.className,
    ).toContain("shrink-0");
  });
});
