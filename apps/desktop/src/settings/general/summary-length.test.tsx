import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setSummaryLength: vi.fn(),
  value: "balanced",
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.value,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setSummaryLength,
}));

vi.mock("@anlg/ui/components/ui/select", async () => {
  const React = await import("react");
  const SelectContext = React.createContext({
    value: "",
    onValueChange: (_value: string) => {},
  });

  return {
    Select: ({
      value,
      onValueChange,
      children,
    }: {
      value: string;
      onValueChange: (value: string) => void;
      children: React.ReactNode;
    }) => (
      <SelectContext.Provider value={{ value, onValueChange }}>
        {children}
      </SelectContext.Provider>
    ),
    SelectContent: ({ children }: { children: React.ReactNode }) => (
      <div>{children}</div>
    ),
    SelectItem: ({
      value,
      children,
    }: {
      value: string;
      children: React.ReactNode;
    }) => {
      const select = React.useContext(SelectContext);
      return (
        <button onClick={() => select.onValueChange(value)}>{children}</button>
      );
    },
    SelectTrigger: ({
      children,
      ...props
    }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
      <button {...props}>{children}</button>
    ),
    SelectValue: () => {
      const select = React.useContext(SelectContext);
      return <span>{select.value}</span>;
    },
  };
});

import { SummaryLengthSelector } from "./summary-length";

describe("SummaryLengthSelector", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.value = "balanced";
  });

  it("shows all three modes and persists the selected mode", () => {
    render(<SummaryLengthSelector />);

    expect(
      screen.getByRole("button", { name: "Summary length" }).textContent,
    ).toBe("balanced");
    expect(screen.getByText("Balanced")).toBeTruthy();
    expect(screen.getByText("Detailed")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Crisp" }));

    expect(mocks.setSummaryLength).toHaveBeenCalledWith("crisp");
  });

  it("falls back to the existing detailed behavior for invalid values", () => {
    mocks.value = "invalid";

    render(<SummaryLengthSelector />);

    expect(
      screen.getByRole("button", { name: "Summary length" }).textContent,
    ).toBe("detailed");
  });
});
