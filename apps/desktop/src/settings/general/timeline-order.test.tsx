import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setTimelineOrder: vi.fn(),
  timelineOrder: "upcoming_first",
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setTimelineOrder,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.timelineOrder,
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

import { TimelineOrderSelector } from "./timeline-order";

describe("TimelineOrderSelector", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.timelineOrder = "upcoming_first";
  });

  it("shows the current sidebar timeline order", () => {
    render(<TimelineOrderSelector />);

    expect(screen.getByText("Sidebar timeline")).toBeTruthy();
    expect(screen.getByText("upcoming_first")).toBeTruthy();
  });

  it("saves chronological order", () => {
    render(<TimelineOrderSelector />);

    fireEvent.click(screen.getByRole("button", { name: "Oldest first" }));

    expect(mocks.setTimelineOrder).toHaveBeenCalledWith("chronological");
  });
});
