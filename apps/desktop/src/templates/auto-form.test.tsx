import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getTemplateSource: vi.fn(),
  renderTemplate: vi.fn(),
  setSettingValue: vi.fn(),
  toastError: vi.fn(),
  inferSummaryFormat: vi.fn(),
  model: { modelId: "test-model" },
  billing: {
    isPro: true,
    isUpgradingToPro: false,
    upgradeToPro: vi.fn(),
  },
  values: {
    auto_summary_prompt: "",
    selected_template_id: "",
  } as Record<string, string>,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (
      input: TemplateStringsArray | { message?: string } | string,
      ...values: unknown[]
    ) => {
      if (typeof input === "string") return input;
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

vi.mock("@anlg/editor/prompt", async () => {
  const React = await import("react");

  return {
    PromptEditor: React.forwardRef(function PromptEditorMock(
      {
        ariaLabel,
        initialValue,
        onBlur,
        onChange,
        readOnly,
      }: {
        ariaLabel: string;
        initialValue: string;
        onBlur?: () => void;
        onChange: (value: string) => void;
        readOnly?: boolean;
      },
      ref: React.ForwardedRef<{
        insertToken: (name: string) => void;
        setValue: (value: string) => void;
      }>,
    ) {
      const [value, setValue] = React.useState(initialValue);
      const update = (next: string) => {
        setValue(next);
        onChange(next);
      };

      React.useImperativeHandle(ref, () => ({
        insertToken: (name) => update(`${value}\n{{ ${name} }}`),
        setValue: update,
      }));

      return (
        <textarea
          aria-label={ariaLabel}
          readOnly={readOnly}
          value={value}
          onBlur={onBlur}
          onChange={(event) => update(event.target.value)}
        />
      );
    }),
  };
});

vi.mock("@anlg/plugin-template", () => ({
  commands: {
    getTemplateSource: mocks.getTemplateSource,
    render: mocks.renderTemplate,
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("./auto-format-inference", () => ({
  inferSummaryFormat: mocks.inferSummaryFormat,
  MAX_FORMAT_EXAMPLE_LENGTH: 12_000,
  MAX_FORMAT_EXAMPLES: 3,
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => mocks.model,
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.values[key] ?? "",
}));

import { AutoFormatForm, AutoTemplateDetails } from "./auto-form";

const defaultFormat = "- Use # headings.\n- Use bullet points.";

function renderWithQueryClient(node: ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{node}</QueryClientProvider>,
  );
}

describe("Auto format editor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.values.auto_summary_prompt = "";
    mocks.values.selected_template_id = "";
    mocks.billing.isPro = true;
    mocks.billing.isUpgradingToPro = false;
    mocks.billing.upgradeToPro.mockClear();
    mocks.getTemplateSource.mockResolvedValue({
      status: "ok",
      data: defaultFormat,
    });
    mocks.renderTemplate.mockResolvedValue({ status: "ok", data: "rendered" });
    mocks.setSettingValue.mockResolvedValue(undefined);
    mocks.inferSummaryFormat.mockResolvedValue(
      "- Begin with decisions.\n- Use concise bullets.",
    );
  });

  afterEach(cleanup);

  it("loads only the built-in format requirements", async () => {
    renderWithQueryClient(<AutoTemplateDetails />);

    expect(
      (await screen.findByRole("textbox", {
        name: "Auto summary format",
      })) as HTMLTextAreaElement,
    ).toHaveProperty("value", defaultFormat);
    expect(mocks.getTemplateSource).toHaveBeenCalledWith("enhanceFormat");
    expect(screen.queryByText("Variables")).toBeNull();
  });

  it("keeps the format visible and read-only for Free users", () => {
    mocks.billing.isPro = false;

    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="" />,
    );

    expect(
      screen.getByRole("textbox", {
        name: "Auto summary format",
      }) as HTMLTextAreaElement,
    ).toHaveProperty("readOnly", true);
    expect(
      screen.getByText(
        "Preview the summary format, then upgrade to Pro to customize it.",
      ),
    ).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Get Pro to customize" }),
    );

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("routes example generation to the Pro upgrade for Free users", () => {
    mocks.billing.isPro = false;

    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="" />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Improve with examples" }),
    );

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
    expect(
      screen.queryByRole("dialog", { name: "Improve summary format" }),
    ).toBeNull();
  });

  it("generates an editable format from up to three transient examples", async () => {
    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="" />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Improve with examples" }),
    );
    fireEvent.change(
      screen.getByRole("textbox", { name: "Example summary 1" }),
      { target: { value: "# Decisions\n- Ship the change" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Add example" }));
    fireEvent.change(
      screen.getByRole("textbox", { name: "Example summary 2" }),
      { target: { value: "# Decisions\n- Launch next week" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Add example" }));
    fireEvent.change(
      screen.getByRole("textbox", { name: "Example summary 3" }),
      { target: { value: "# Decisions\n- Keep concise" } },
    );

    expect(screen.getByRole("button", { name: "Add example" })).toHaveProperty(
      "disabled",
      true,
    );
    fireEvent.click(screen.getByRole("button", { name: "Improve format" }));

    await waitFor(() =>
      expect(mocks.inferSummaryFormat).toHaveBeenCalledWith({
        model: mocks.model,
        examples: [
          "# Decisions\n- Ship the change",
          "# Decisions\n- Launch next week",
          "# Decisions\n- Keep concise",
        ],
      }),
    );
    expect(
      screen.getByRole("textbox", {
        name: "Auto summary format",
      }) as HTMLTextAreaElement,
    ).toHaveProperty(
      "value",
      "- Begin with decisions.\n- Use concise bullets.",
    );
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("accepts Markdown files as example summaries", async () => {
    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="" />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Improve with examples" }),
    );
    const file = new File(["# Overview\n- Concise"], "summary.md", {
      type: "text/markdown",
    });
    Object.defineProperty(file, "text", {
      value: vi.fn().mockResolvedValue("# Overview\n- Concise"),
    });

    fireEvent.change(document.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });

    await waitFor(() =>
      expect(
        screen.getByRole("textbox", { name: "Example summary 1" }),
      ).toHaveProperty("value", "# Overview\n- Concise"),
    );
  });

  it("validates and saves a customized format", async () => {
    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="" />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Auto summary format" }),
      { target: { value: "Write a concise narrative." } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mocks.renderTemplate).toHaveBeenCalledWith({
        enhanceSystem: {
          language: "en",
          formatOverride: "Write a concise narrative.",
        },
      }),
    );
    expect(mocks.setSettingValue).toHaveBeenCalledWith(
      "auto_summary_prompt",
      "Write a concise narrative.",
    );
  });

  it("stores the default-equivalent source as an empty override", async () => {
    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="Custom" />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Auto summary format" }),
      { target: { value: `  ${defaultFormat}\n` } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "auto_summary_prompt",
        "",
      ),
    );
  });

  it("toasts Jinja validation errors without saving", async () => {
    mocks.renderTemplate.mockResolvedValue({
      status: "error",
      error: "unknown variables: customer",
    });
    renderWithQueryClient(
      <AutoFormatForm defaultFormat={defaultFormat} formatOverride="" />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Auto summary format" }),
      { target: { value: "Hello {{ customer }}" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith(
        "unknown variables: customer",
      ),
    );
    expect(screen.queryByRole("alert")).toBeNull();
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("resets a customized format to the built-in source", async () => {
    renderWithQueryClient(
      <AutoFormatForm
        defaultFormat={defaultFormat}
        formatOverride="Custom format"
      />,
    );

    expect(
      screen.queryByRole("menuitem", { name: "Reset to default format" }),
    ).toBeNull();
    fireEvent.pointerDown(
      screen.getByRole("button", { name: "Template actions" }),
      { button: 0, ctrlKey: false },
    );
    fireEvent.click(
      await screen.findByRole("menuitem", { name: "Reset to default format" }),
    );

    await waitFor(() =>
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "auto_summary_prompt",
        "",
      ),
    );
    expect(
      screen.getByRole("textbox", {
        name: "Auto summary format",
      }) as HTMLTextAreaElement,
    ).toHaveProperty("value", defaultFormat);
  });

  it("extracts the editable portion from a legacy full prompt", () => {
    renderWithQueryClient(
      <AutoFormatForm
        defaultFormat={defaultFormat}
        formatOverride={`# General Instructions

Protected instructions.

# Format Requirements

- Start with decisions.

# About Notes

Protected note guidance.

# Guidelines

Protected guidelines.`}
      />,
    );

    expect(
      screen.getByRole("textbox", {
        name: "Auto summary format",
      }) as HTMLTextAreaElement,
    ).toHaveProperty("value", "- Start with decisions.");
    expect(screen.queryByText("Protected instructions.")).toBeNull();
  });
});
