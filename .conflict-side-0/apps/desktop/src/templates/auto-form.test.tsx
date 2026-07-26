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
          value={value}
          onBlur={onBlur}
          onChange={(event) => update(event.target.value)}
        />
      );
    }),
  };
});

vi.mock("@hypr/plugin-template", () => ({
  commands: {
    getTemplateSource: mocks.getTemplateSource,
    render: mocks.renderTemplate,
  },
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.values[key] ?? "",
}));

import { AutoPromptForm, AutoTemplateDetails } from "./auto-form";

const defaultPrompt =
  "Today is {{ current_date }}. Write the summary in {{ language }}.";

function renderWithQueryClient(node: ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{node}</QueryClientProvider>,
  );
}

describe("Auto prompt editor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.values.auto_summary_prompt = "";
    mocks.values.selected_template_id = "";
    mocks.getTemplateSource.mockResolvedValue({
      status: "ok",
      data: defaultPrompt,
    });
    mocks.renderTemplate.mockResolvedValue({ status: "ok", data: "rendered" });
    mocks.setSettingValue.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  it("loads the built-in source and shows supported variables and context", async () => {
    renderWithQueryClient(<AutoTemplateDetails />);

    expect(
      (await screen.findByRole("textbox", {
        name: "Auto summary prompt",
      })) as HTMLTextAreaElement,
    ).toHaveProperty("value", defaultPrompt);
    expect(screen.getByRole("button", { name: /Current date/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Language/ })).toBeTruthy();
    expect(screen.getByText("Meeting notes")).toBeTruthy();
    expect(screen.getByText("Transcript")).toBeTruthy();
  });

  it("inserts supported variables as canonical prompt tokens", () => {
    renderWithQueryClient(
      <AutoPromptForm defaultPrompt={defaultPrompt} promptOverride="" />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Language/ }));

    expect(
      screen.getByRole("textbox", {
        name: "Auto summary prompt",
      }) as HTMLTextAreaElement,
    ).toHaveProperty("value", `${defaultPrompt}\n{{ language }}`);
  });

  it("validates and saves a customized prompt", async () => {
    renderWithQueryClient(
      <AutoPromptForm defaultPrompt={defaultPrompt} promptOverride="" />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Auto summary prompt" }),
      { target: { value: "Write in {{ language }}." } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mocks.renderTemplate).toHaveBeenCalledWith({
        enhanceSystem: {
          language: "en",
          promptOverride: "Write in {{ language }}.",
        },
      }),
    );
    expect(mocks.setSettingValue).toHaveBeenCalledWith(
      "auto_summary_prompt",
      "Write in {{ language }}.",
    );
  });

  it("stores the default-equivalent source as an empty override", async () => {
    renderWithQueryClient(
      <AutoPromptForm defaultPrompt={defaultPrompt} promptOverride="Custom" />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Auto summary prompt" }),
      { target: { value: `  ${defaultPrompt}\n` } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "auto_summary_prompt",
        "",
      ),
    );
  });

  it("shows Jinja validation errors without saving", async () => {
    mocks.renderTemplate.mockResolvedValue({
      status: "error",
      error: "unknown variables: customer",
    });
    renderWithQueryClient(
      <AutoPromptForm defaultPrompt={defaultPrompt} promptOverride="" />,
    );

    fireEvent.change(
      screen.getByRole("textbox", { name: "Auto summary prompt" }),
      { target: { value: "Hello {{ customer }}" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect((await screen.findByRole("alert")).textContent).toContain(
      "unknown variables: customer",
    );
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("resets a customized prompt to the built-in source", async () => {
    renderWithQueryClient(
      <AutoPromptForm
        defaultPrompt={defaultPrompt}
        promptOverride="Custom prompt"
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Reset to Anarlog default" }),
    );

    await waitFor(() =>
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "auto_summary_prompt",
        "",
      ),
    );
    expect(
      screen.getByRole("textbox", {
        name: "Auto summary prompt",
      }) as HTMLTextAreaElement,
    ).toHaveProperty("value", defaultPrompt);
  });
});
