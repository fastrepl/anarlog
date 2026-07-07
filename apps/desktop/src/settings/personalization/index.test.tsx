import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
  }: {
    children?: ReactNode;
    id?: string;
    message?: string;
  }) => <>{children ?? message ?? id}</>,
  useLingui: () => ({
    t: (
      input: TemplateStringsArray | { message?: string } | string,
      ...values: unknown[]
    ) => {
      if (typeof input === "string") {
        return input;
      }

      if (Array.isArray(input)) {
        return (input as readonly string[]).reduce(
          (message: string, part: string, index: number) =>
            `${message}${part}${index < values.length ? String(values[index]) : ""}`,
          "",
        );
      }

      return (input as { message?: string }).message ?? "";
    },
  }),
}));

import { DictionarySettings } from "./index";

describe("DictionarySettings", () => {
  afterEach(() => {
    cleanup();
  });

  it("only shows the input when the dictionary is empty", () => {
    render(<DictionarySettings terms={[]} onSave={vi.fn()} />);

    const input = screen.getByRole("textbox");

    expect(input).toBeTruthy();
    expect(input.closest("[data-slot='input-group']")?.className).toContain(
      "border-border",
    );
    expect(screen.queryByText("Examples")).toBeNull();
    expect(screen.queryByText("FastConformer")).toBeNull();
  });

  it("adds entered terms and keeps them normalized", async () => {
    const onSave = vi.fn();
    render(<DictionarySettings terms={["Anarlog"]} onSave={onSave} />);

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: " FastConformer, Parakeet TDT " },
    });
    const addButton = screen.getByRole("button", {
      name: "Add",
    }) as HTMLButtonElement;
    await waitFor(() => expect(addButton.disabled).toBe(false));
    fireEvent.click(addButton);

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        JSON.stringify(["Anarlog", "FastConformer", "Parakeet TDT"]),
      ),
    );
  });

  it("removes saved terms", () => {
    const onSave = vi.fn();
    render(
      <DictionarySettings
        terms={["Anarlog", "Parakeet TDT"]}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Remove Anarlog" }));

    expect(onSave).toHaveBeenCalledWith(JSON.stringify(["Parakeet TDT"]));
  });

  it("does not enable adding duplicate terms", async () => {
    const onSave = vi.fn();
    render(<DictionarySettings terms={["Anarlog"]} onSave={onSave} />);

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "anarlog" },
    });

    const addButton = screen.getByRole("button", {
      name: "Add",
    }) as HTMLButtonElement;
    await waitFor(() => expect(addButton.disabled).toBe(true));
    fireEvent.click(addButton);

    expect(onSave).not.toHaveBeenCalled();
  });

  it("emphasizes the add button while typing", async () => {
    render(<DictionarySettings terms={["Anarlog"]} onSave={vi.fn()} />);

    const addButton = screen.getByRole("button", {
      name: "Add",
    }) as HTMLButtonElement;

    expect(addButton.className).not.toContain("bg-[#2f6f68]");

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "FastConformer" },
    });

    await waitFor(() => expect(addButton.className).toContain("bg-[#2f6f68]"));
  });

  it("shows relevant saved terms while typing", async () => {
    render(
      <DictionarySettings
        terms={["Anarlog", "FastConformer", "Parakeet TDT"]}
        onSave={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "fast" },
    });

    await waitFor(() => expect(screen.getByText("FastConformer")).toBeTruthy());
    expect(screen.queryByText("Anarlog")).toBeNull();
    expect(screen.queryByText("Parakeet TDT")).toBeNull();
  });

  it("shows no match below the input when typed text has no saved match", async () => {
    render(
      <DictionarySettings
        terms={["Anarlog", "FastConformer"]}
        onSave={vi.fn()}
      />,
    );

    expect(screen.queryByText("No match")).toBeNull();

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "parakeet" },
    });

    await waitFor(() => expect(screen.getByText("No match")).toBeTruthy());
    expect(screen.queryByText("Anarlog")).toBeNull();
    expect(screen.queryByText("FastConformer")).toBeNull();
  });
});
