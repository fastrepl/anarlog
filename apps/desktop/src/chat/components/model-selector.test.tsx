import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  selection: { providerId: "openai", modelId: "gpt-5.4" },
  save: vi.fn(),
  toastError: vi.fn(),
  customConfigured: false,
  catalogs: {
    openai: ["gpt-5.4"],
    anthropic: ["claude-sonnet-5"],
    anarlog: ["Auto", "~anthropic/claude-sonnet-latest"],
  },
}));

vi.mock("~/ai/hooks", async () => ({
  useLLMSelection: () => mocks.selection,
  useLLMConnection: () => ({ status: { status: "success" } }),
  useModelMetadata: (await import("~/ai/hooks/useModelMetadata"))
    .useModelMetadata,
}));
vi.mock("~/settings/ai/llm/shared", () => ({
  PROVIDERS: [
    { id: "openai", displayName: "OpenAI" },
    { id: "anthropic", displayName: "Anthropic" },
    { id: "anarlog", displayName: "Anarlog" },
    { id: "custom", displayName: "Custom" },
  ],
}));
vi.mock("~/settings/ai/llm/configured-providers", () => ({
  useConfiguredLlmProviders: () => ({
    isReady: true,
    providers: {
      openai: {
        configured: true,
        listModels: async () => ({
          models: mocks.catalogs.openai,
          ignored: [],
        }),
      },
      anthropic: {
        configured: true,
        listModels: async () => ({
          models: mocks.catalogs.anthropic,
          ignored: [],
        }),
      },
      anarlog: {
        configured: true,
        listModels: async () => ({
          models: mocks.catalogs.anarlog,
          ignored: [],
        }),
      },
      custom: {
        configured: mocks.customConfigured,
        listModels: async () => ({ models: [], ignored: [] }),
      },
    },
  }),
}));
vi.mock("~/settings/queries", () => ({ setSettingValues: mocks.save }));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.toastError },
}));

import { ChatModelSelector } from "./model-selector";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.selection = { providerId: "openai", modelId: "gpt-5.4" };
  mocks.save.mockResolvedValue(undefined);
  mocks.customConfigured = false;
  HTMLElement.prototype.scrollIntoView = vi.fn();
});
afterEach(cleanup);

function mount(disabled = false) {
  const client = new QueryClient();
  const view = (isDisabled: boolean) => (
    <QueryClientProvider client={client}>
      <ChatModelSelector disabled={isDisabled} />
    </QueryClientProvider>
  );
  return { ...render(view(disabled)), view };
}

it("opens providers before models, supports keyboard search, and saves chat-only choices for BYOK and hosted models", async () => {
  const { rerender, view } = mount(true);
  const getTrigger = () =>
    screen.getByRole("button", { name: "Chat model: OpenAI, GPT 5.4" });
  expect((getTrigger() as HTMLButtonElement).disabled).toBe(true);
  rerender(view(false));
  fireEvent.keyDown(getTrigger(), { key: "ArrowDown" });
  await screen.findByRole("menuitem", { name: "Anthropic" });
  expect(screen.getByRole("menuitem", { name: "OpenAI" })).toBeTruthy();
  expect(screen.queryByRole("menuitem", { name: "Custom" })).toBeNull();
  expect(
    screen.queryByRole("menuitem", { name: "Claude Sonnet 5" }),
  ).toBeNull();
  const search = screen.getByRole("textbox", { name: "Search models" });
  await waitFor(() => expect(document.activeElement).toBe(search));
  fireEvent.change(search, { target: { value: "sonnet" } });
  const matchingModel = await screen.findByRole("menuitem", {
    name: "Claude Sonnet 5",
  });
  expect(screen.getByRole("group", { name: "Anthropic" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "Anarlog" })).toBeTruthy();
  expect(screen.queryByRole("group", { name: "OpenAI" })).toBeNull();
  expect(screen.queryByRole("menuitem", { name: "Anthropic" })).toBeNull();
  expect(screen.getAllByRole("menu")).toHaveLength(1);
  fireEvent.keyDown(search, { key: "ArrowDown" });
  expect(document.activeElement).toBe(matchingModel);
  fireEvent.keyDown(matchingModel, { key: "Enter" });
  await waitFor(() =>
    expect(screen.queryByPlaceholderText("Search models")).toBeNull(),
  );
  expect(mocks.save.mock.calls[0][0]).toEqual({
    chat_llm_provider: "anthropic",
    chat_llm_model: "claude-sonnet-5",
  });

  fireEvent.keyDown(getTrigger(), { key: "ArrowDown" });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Anarlog" }));
  const hostedModel = await screen.findByRole("menuitem", {
    name: "Claude Sonnet",
  });
  expect(screen.queryByRole("menuitem", { name: "Pro (Cloud)" })).toBeNull();
  expect(screen.queryByText(/^(Default|Auto)$/)).toBeNull();
  fireEvent.click(hostedModel);
  await waitFor(() =>
    expect(mocks.save.mock.calls[1]?.[0]).toEqual({
      chat_llm_provider: "anarlog",
      chat_llm_model: "~anthropic/claude-sonnet-latest",
    }),
  );
});

it("keeps the current model and the picker open when saving fails", async () => {
  mocks.save.mockRejectedValue(new Error("disk unavailable"));
  mount();
  fireEvent.keyDown(
    screen.getByRole("button", { name: "Chat model: OpenAI, GPT 5.4" }),
    { key: "ArrowDown" },
  );
  fireEvent.click(await screen.findByRole("menuitem", { name: "Anthropic" }));
  fireEvent.click(
    await screen.findByRole("menuitem", { name: "Claude Sonnet 5" }),
  );
  await waitFor(() => expect(mocks.toastError).toHaveBeenCalled());
  expect(screen.getByPlaceholderText("Search models")).toBeTruthy();
  expect(
    screen.getByRole("menu", { name: "Chat model: OpenAI, GPT 5.4" }),
  ).toBeTruthy();
});

it("selects a pasted custom model without discovery, trimming spaces while preserving case", async () => {
  mocks.customConfigured = true;
  mocks.selection = { providerId: "custom", modelId: "summary-small" };
  mount();
  fireEvent.keyDown(
    screen.getByRole("button", { name: "Chat model: Custom, Summary Small" }),
    { key: "ArrowDown" },
  );
  const modelId = "Chat-Large/V2";
  fireEvent.change(
    await screen.findByRole("textbox", { name: "Search models" }),
    { target: { value: `  ${modelId}  ` } },
  );
  fireEvent.click(await screen.findByRole("menuitem", { name: modelId }));
  await waitFor(() =>
    expect(mocks.save.mock.calls[0]?.[0]).toEqual({
      chat_llm_provider: "custom",
      chat_llm_model: modelId,
    }),
  );
});

it("selects a search result after expanding a large catalog", async () => {
  mocks.catalogs.openai = Array.from(
    { length: 120 },
    (_, index) => `gpt-6-luna-${index}`,
  );
  mount();
  fireEvent.keyDown(
    screen.getByRole("button", { name: "Chat model: OpenAI, GPT 5.4" }),
    { key: "ArrowDown" },
  );
  fireEvent.change(
    await screen.findByRole("textbox", { name: "Search models" }),
    { target: { value: "luna" } },
  );
  fireEvent.click(
    await screen.findByRole("menuitem", {
      name: "Show more models",
    }),
  );
  fireEvent.click(
    await screen.findByRole("menuitem", {
      name: "GPT 6 Luna 60",
    }),
  );
  await waitFor(() =>
    expect(mocks.save.mock.calls[0]?.[0]).toEqual({
      chat_llm_provider: "openai",
      chat_llm_model: "gpt-6-luna-60",
    }),
  );
});
