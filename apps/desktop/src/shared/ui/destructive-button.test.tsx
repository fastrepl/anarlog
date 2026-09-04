import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Button } from "@anlg/ui/components/ui/button";

import { finishDestructiveButtonHold } from "~/test-utils/destructive-button";

describe("destructive Button", () => {
  afterEach(cleanup);

  it("requires a completed pointer hold", () => {
    const onClick = vi.fn();
    render(
      <Button variant="destructive" onClick={onClick}>
        Delete
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Delete" });

    fireEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();

    fireEvent.pointerDown(button, { button: 0, isPrimary: true });
    expect(button.dataset.holdState).toBe("holding");

    finishDestructiveButtonHold(button);

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(button.dataset.holdState).toBe("idle");
  });

  it("cancels a pointer hold when released early", () => {
    const onClick = vi.fn();
    render(
      <Button variant="destructive" onClick={onClick}>
        Delete
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Delete" });

    fireEvent.pointerDown(button, { button: 0, isPrimary: true });
    fireEvent.pointerUp(button);
    finishDestructiveButtonHold(button);
    fireEvent.click(button);

    expect(onClick).not.toHaveBeenCalled();
    expect(button.dataset.holdState).toBe("idle");
  });

  it("supports holding Enter from the keyboard", () => {
    const onClick = vi.fn();
    render(
      <Button variant="destructive" onClick={onClick}>
        Delete
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Delete" });

    fireEvent.keyDown(button, { key: "Enter" });
    expect(button.dataset.holdState).toBe("holding");

    finishDestructiveButtonHold(button);

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("cancels a keyboard hold when focus leaves", () => {
    const onClick = vi.fn();
    render(
      <Button variant="destructive" onClick={onClick}>
        Delete
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Delete" });

    fireEvent.keyDown(button, { key: "Enter" });
    fireEvent.blur(button);
    finishDestructiveButtonHold(button);

    expect(onClick).not.toHaveBeenCalled();
    expect(button.dataset.holdState).toBe("idle");
  });

  it("keeps non-destructive buttons clickable", () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Save</Button>);

    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
