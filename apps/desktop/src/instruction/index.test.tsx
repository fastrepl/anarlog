import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  close: vi.fn().mockResolvedValue(undefined),
  isMaximized: vi.fn().mockResolvedValue(false),
  minimize: vi.fn().mockResolvedValue(undefined),
  onResized: vi.fn().mockResolvedValue(vi.fn()),
  platform: "macos",
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: mocks.close,
    isMaximized: mocks.isMaximized,
    minimize: mocks.minimize,
    onResized: mocks.onResized,
    toggleMaximize: mocks.toggleMaximize,
  }),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: vi.fn() },
}));

vi.mock("~/auth", () => ({
  useAuth: () => null,
}));

import { InstructionScreen } from "~/instruction";

afterEach(() => {
  cleanup();
  mocks.platform = "macos";
  mocks.close.mockClear();
  mocks.minimize.mockClear();
  mocks.toggleMaximize.mockClear();
});

describe("InstructionScreen billing", () => {
  it("frames checkout as a Pro upgrade", () => {
    render(
      <InstructionScreen
        type="billing"
        url="https://anarlog.so/app/checkout"
        onBack={() => {}}
        onClose={() => {}}
      />,
    );

    expect(screen.getByText("Upgrade to Pro")).toBeTruthy();
    expect(
      screen.getByText(
        "Finish checkout in your browser to unlock more, then return to Anarlog.",
      ),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: /Reopen checkout page/ }),
    ).toBeTruthy();
  });

  it.each(["windows", "linux"])(
    "shows usable window controls on %s",
    (platform) => {
      mocks.platform = platform;
      const onClose = vi.fn();
      render(
        <InstructionScreen
          type="billing"
          onBack={() => {}}
          onClose={onClose}
        />,
      );

      fireEvent.click(screen.getByRole("button", { name: "Minimize" }));
      fireEvent.click(screen.getByRole("button", { name: "Maximize" }));
      fireEvent.click(screen.getByRole("button", { name: "Close" }));

      expect(mocks.minimize).toHaveBeenCalledOnce();
      expect(mocks.toggleMaximize).toHaveBeenCalledOnce();
      expect(onClose).toHaveBeenCalledOnce();
      expect(mocks.close).not.toHaveBeenCalled();
    },
  );

  it("keeps macOS native window controls instead of duplicating them", () => {
    render(
      <InstructionScreen type="billing" onBack={() => {}} onClose={() => {}} />,
    );

    for (const name of ["Minimize", "Maximize", "Close"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
  });
});
