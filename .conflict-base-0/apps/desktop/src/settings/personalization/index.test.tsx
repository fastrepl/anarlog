import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const settingsMocks = vi.hoisted(() => ({
  setValue: vi.fn(),
  setValues: vi.fn(),
}));

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

vi.mock("@hypr/editor/prompt", async () => {
  const React = await import("react");

  return {
    PromptEditor: React.forwardRef(function PromptEditorMock(
      {
        ariaLabel,
        initialValue,
        onBlur,
        onChange,
      }: {
        ariaLabel: string;
        initialValue: string;
        onBlur?: () => void;
        onChange: (value: string) => void;
      },
      ref: React.ForwardedRef<{
        insertToken: (name: "template") => void;
        setValue: (value: string) => void;
      }>,
    ) {
      React.useImperativeHandle(ref, () => ({
        insertToken: () => onChange(`${initialValue}{{ template }}`),
        setValue: onChange,
      }));

      return (
        <textarea
          aria-label={ariaLabel}
          value={initialValue}
          onBlur={onBlur}
          onChange={(event) => onChange(event.target.value)}
        />
      );
    }),
  };
});

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => settingsMocks.setValue,
  useSetSettingValues: () => settingsMocks.setValues,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "personalization_dictionary_terms"
      ? []
      : key === "custom_summary_instructions_token_aware"
        ? false
        : "",
}));

import {
  DictionarySettings,
  SettingsPersonalization,
  SummaryInstructionsSettings,
} from "./index";

import { DEFAULT_SUMMARY_PROMPT } from "~/shared/summary-prompt";

describe("SettingsPersonalization", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("shows the dictionary before the summary instructions", () => {
    render(<SettingsPersonalization />);

    expect(
      screen
        .getAllByRole("heading", { level: 2 })
        .map((heading) => heading.textContent)
        .slice(-2),
    ).toEqual(["Dictionary", "Summary instructions"]);
  });
});

describe("SummaryInstructionsSettings", () => {
  afterEach(() => {
    cleanup();
  });

  it("explains how the template chip controls selected templates", () => {
    render(
      <SummaryInstructionsSettings
        instructions={"Keep it brief\n\n{{ template }}"}
        onSave={vi.fn()}
      />,
    );

    expect(
      screen.getByText(
        /The Template chip inserts the selected template. Remove it to ignore templates/,
      ),
    ).toBeTruthy();
    expect(
      (
        screen.getByRole("textbox", {
          name: "Summary instructions",
        }) as HTMLTextAreaElement
      ).value,
    ).toBe("Keep it brief\n\n{{ template }}");
  });

  it("shows variables below the editor", () => {
    render(<SummaryInstructionsSettings instructions="" onSave={vi.fn()} />);

    const editor = screen.getByRole("textbox", {
      name: "Summary instructions",
    });
    const variables = screen.getByText("Variables");

    expect(
      editor.compareDocumentPosition(variables) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("disables reset when the built-in prompt is already active", () => {
    render(
      <SummaryInstructionsSettings
        instructions={DEFAULT_SUMMARY_PROMPT}
        onSave={vi.fn()}
      />,
    );

    expect(
      (
        screen.getByRole("button", {
          name: "Reset to default",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });

  it("saves trimmed instructions explicitly", async () => {
    const onSave = vi.fn();
    render(<SummaryInstructionsSettings instructions="" onSave={onSave} />);

    fireEvent.change(
      screen.getByRole("textbox", { name: "Summary instructions" }),
      { target: { value: "  Use a short executive summary.  " } },
    );

    const saveButton = screen.getByRole("button", {
      name: "Save",
    }) as HTMLButtonElement;
    await waitFor(() => expect(saveButton.disabled).toBe(false));
    fireEvent.click(saveButton);

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith("Use a short executive summary."),
    );
  });

  it("resets saved instructions to the built-in behavior", () => {
    const onSave = vi.fn();
    render(
      <SummaryInstructionsSettings
        instructions="Use a table"
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset to default" }));

    expect(onSave).toHaveBeenCalledWith("");
    expect(
      (
        screen.getByRole("textbox", {
          name: "Summary instructions",
        }) as HTMLTextAreaElement
      ).value,
    ).toBe(DEFAULT_SUMMARY_PROMPT);
  });

  it("warns when the template token is removed", () => {
    render(
      <SummaryInstructionsSettings
        instructions="Use headings\n\n{{ template }}"
        onSave={vi.fn()}
      />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Summary instructions" }),
      { target: { value: "Do not use headings." } },
    );

    expect(screen.getByText(/Selected templates will be ignored/)).toBeTruthy();
    expect(
      screen.queryByText(/take priority over the selected template/),
    ).toBeNull();
    expect(
      (screen.getByRole("button", { name: "Template" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });

  it("can insert the template token back into the prompt", () => {
    render(
      <SummaryInstructionsSettings
        instructions="Use a short summary. "
        onSave={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Template" }));

    expect(
      (
        screen.getByRole("textbox", {
          name: "Summary instructions",
        }) as HTMLTextAreaElement
      ).value,
    ).toContain("{{ template }}");
    expect(screen.queryByText(/Selected templates will be ignored/)).toBeNull();
  });
});

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

  it("uses an inverted add button while typing", async () => {
    render(<DictionarySettings terms={["Anarlog"]} onSave={vi.fn()} />);

    const addButton = screen.getByRole("button", {
      name: "Add",
    }) as HTMLButtonElement;

    expect(addButton.className).not.toContain("bg-[#2f6f68]");
    expect(addButton.className).not.toContain("bg-black");

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "FastConformer" },
    });

    await waitFor(() => expect(addButton.className).toContain("bg-black"));
    expect(addButton.className).toContain("text-white");
    expect(addButton.className).toContain("dark:bg-white");
    expect(addButton.className).toContain("dark:text-black");
    expect(addButton.className).not.toContain("bg-[#2f6f68]");
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
