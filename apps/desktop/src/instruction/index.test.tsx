import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

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
});

describe("InstructionScreen billing", () => {
  it("frames checkout as a Pro upgrade", () => {
    render(
      <InstructionScreen
        type="billing"
        url="https://anarlog.so/app/checkout"
        onBack={() => {}}
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
});
