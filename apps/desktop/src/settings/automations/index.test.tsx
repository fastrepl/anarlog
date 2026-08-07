import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The real iconify-icon web component renders asynchronously via timers that
// can fire after the test environment is torn down ("document is not
// defined" unhandled errors), so render an inert element instead.
vi.mock("@iconify-icon/react", () => ({
  Icon: (props: Record<string, unknown>) =>
    createElement("iconify-icon", props),
}));

const mocks = vi.hoisted(() => ({
  billing: {
    isPro: true,
    isReady: true,
    upgradeToPro: vi.fn(),
  },
  setSettingValue: vi.fn(() => Promise.resolve()),
  storedDraft: "",
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
  setSettingValues: mocks.setSettingValue,
  useStoredSettingValue: () => ({
    value: mocks.storedDraft,
    hasValue: Boolean(mocks.storedDraft),
  }),
  useStoredSettingValues: () => ({ values: {}, hasValues: new Set() }),
}));

vi.mock("./starter-config", () => ({
  AutomationLastRunLine: () => null,
  MarkdownExportConfig: () => <div data-testid="config-markdown" />,
  SlackRecapConfig: () => <div data-testid="config-slack" />,
  LinearIssuesConfig: () => <div data-testid="config-linear" />,
  NotionUpdateConfig: () => <div data-testid="config-notion" />,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

import { SettingsAutomations } from ".";

function renderAutomations() {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsAutomations />
    </QueryClientProvider>,
  );
}

describe("SettingsAutomations", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.billing.isPro = true;
    mocks.billing.isReady = true;
    mocks.billing.upgradeToPro.mockClear();
    mocks.setSettingValue.mockClear();
    mocks.storedDraft = "";
    mocks.toastError.mockClear();
    mocks.toastSuccess.mockClear();
  });

  it("turns a starter into an inspectable deterministic draft", () => {
    renderAutomations();

    expect(screen.getByText("No automation draft yet")).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", {
        name: /Share a meeting recap in Slack/,
      }),
    );

    expect(screen.getByText("Use the AI meeting summary")).toBeTruthy();
    expect(screen.getByText("Post to a channel")).toBeTruthy();
    expect(screen.getByTestId("config-slack")).toBeTruthy();
    expect(
      screen.getByRole<HTMLButtonElement>("button", { name: "Test" }).disabled,
    ).toBe(true);
    expect(
      screen.getByRole<HTMLButtonElement>("button", {
        name: "Save & enable",
      }).disabled,
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Preview" }));

    expect(screen.getByText("Expected output")).toBeTruthy();
    expect(
      screen.getByText(/A Slack message with the meeting title and recap/),
    ).toBeTruthy();
  });

  it("uses product marks without icon tiles", () => {
    const { container } = renderAutomations();

    const slackIcon = container.querySelector(
      'iconify-icon[icon="logos:slack-icon"]',
    );

    expect(slackIcon).toBeTruthy();
    expect(
      container.querySelector('iconify-icon[icon="logos:notion-icon"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('iconify-icon[icon="logos:linear-icon"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('img[src="/assets/markdown-mark.svg"]'),
    ).toBeTruthy();
    expect(slackIcon?.parentElement?.className).not.toContain("bg-muted");
    expect(slackIcon?.parentElement?.className).not.toContain("rounded");
  });

  it("shows compact starter rows", () => {
    renderAutomations();

    expect(screen.queryByText("Pro")).toBeNull();
    expect(
      screen.getByText(
        "Automate what happens before, during, or after meetings based on the conditions you choose.",
      ),
    ).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Get started" })).toBeTruthy();
    expect(
      screen.queryByText("Create a draft you can review before saving."),
    ).toBeNull();

    const starter = screen.getByRole("button", {
      name: /Share a meeting recap in Slack/,
    });
    expect(starter.className).not.toContain("border");
    expect(starter.className).not.toContain("p-4");
    expect(starter.querySelector("svg")).toBeTruthy();
  });

  it("saves the selected draft for Pro users", async () => {
    renderAutomations();

    fireEvent.click(
      screen.getByRole("button", {
        name: /Export every meeting as Markdown/,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Save draft" }));

    await waitFor(() => {
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "automation_draft_template",
        "markdown-export",
      );
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Automation draft saved");
  });

  it("offers the Pro upgrade instead of saving on the free plan", () => {
    mocks.billing.isPro = false;
    renderAutomations();

    fireEvent.click(
      screen.getByRole("button", {
        name: /Update project notes in Notion/,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Upgrade to save" }));

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });
});
