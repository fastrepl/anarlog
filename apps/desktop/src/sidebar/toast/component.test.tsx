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
    expect(screen.getByText("Language model needed")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.click(screen.getByRole("button", { name: "Hide" }));

    expect(onAdd).toHaveBeenCalledTimes(1);
    expect(onHide).toHaveBeenCalledTimes(1);
  });
});
