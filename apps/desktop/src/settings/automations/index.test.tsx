import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
  useStoredSettingValue: () => ({
    value: mocks.storedDraft,
    hasValue: Boolean(mocks.storedDraft),
  }),
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

    expect(screen.getByText("Generate a concise recap")).toBeTruthy();
    expect(screen.getByText("Create a Slack canvas")).toBeTruthy();
    expect(screen.getByText("Post to a channel")).toBeTruthy();
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
      screen.getByText(/A Slack canvas with the recap and source note/),
    ).toBeTruthy();
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
