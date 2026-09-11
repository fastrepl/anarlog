import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  isNativeShellAvailable: vi.fn(),
  switchToNativeShell: vi.fn(),
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", async () => {
  const React = await import("react");
  const RadioGroupContext = React.createContext<
    ((value: string) => void) | undefined
  >(undefined);

  return {
    DropdownMenuCheckboxItem: ({ children }: { children: ReactNode }) => (
      <button type="button">{children}</button>
    ),
    DropdownMenuRadioGroup: ({
      children,
      onValueChange,
    }: {
      children: ReactNode;
      onValueChange: (value: string) => void;
    }) => (
      <RadioGroupContext.Provider value={onValueChange}>
        {children}
      </RadioGroupContext.Provider>
    ),
    DropdownMenuRadioItem: ({
      children,
      disabled,
      onSelect,
      value,
    }: {
      children: ReactNode;
      disabled?: boolean;
      onSelect?: (event: React.SyntheticEvent) => void;
      value: string;
    }) => {
      const onValueChange = React.useContext(RadioGroupContext);
      return (
        <button
          type="button"
          disabled={disabled}
          onClick={(event) => {
            onSelect?.(event);
            onValueChange?.(value);
          }}
        >
          {children}
        </button>
      );
    },
  };
});

vi.mock("./menu", () => ({
  MenuGroup: ({
    label,
    description,
    children,
  }: {
    label: string;
    description: string;
    children: ReactNode;
  }) => (
    <section aria-label={label}>
      <p>{description}</p>
      {children}
    </section>
  ),
  MenuHint: ({
    description,
    children,
  }: {
    description: string;
    children: ReactNode;
  }) => <div data-hint={description}>{children}</div>,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValues: () => vi.fn(),
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({ theme: "system" }),
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: {
    isNativeShellAvailable: mocks.isNativeShellAvailable,
    switchToNativeShell: mocks.switchToNativeShell,
  },
}));

import { QuickSettingsMenu } from "./quick-settings";

function renderMenu() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <QuickSettingsMenu />
    </QueryClientProvider>,
  );
}

describe("QuickSettingsMenu shell switch", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("disables the GPUI option when the native shell is unavailable", async () => {
    mocks.isNativeShellAvailable.mockResolvedValue(false);

    renderMenu();

    await waitFor(() =>
      expect(
        (
          screen.getByRole("button", {
            name: "GPUI (native)",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(true),
    );
  });

  it("switches to GPUI when the native shell is selected", async () => {
    mocks.isNativeShellAvailable.mockResolvedValue(true);
    mocks.switchToNativeShell.mockResolvedValue({ status: "ok", data: null });

    renderMenu();

    const item = await screen.findByRole("button", { name: "GPUI (native)" });
    await waitFor(() =>
      expect((item as HTMLButtonElement).disabled).toBe(false),
    );
    fireEvent.click(item);

    await waitFor(() =>
      expect(mocks.switchToNativeShell).toHaveBeenCalledOnce(),
    );
  });
});
