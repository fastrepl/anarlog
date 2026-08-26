import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setValue: vi.fn(),
  value: "me",
  workspaces: [] as { id: string; name: string }[],
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.value,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setValue,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "user-1" } } }),
}));

vi.mock("~/session-sharing/source", () => ({
  useAvailableShareWorkspaces: () => mocks.workspaces,
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
      disabled,
      children,
    }: {
      value: string;
      disabled?: boolean;
      children: React.ReactNode;
    }) => {
      const select = React.useContext(SelectContext);
      return (
        <button disabled={disabled} onClick={() => select.onValueChange(value)}>
          {children}
        </button>
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

import { DefaultMeetingShareAccessSelector } from "./default-share-access";

describe("DefaultMeetingShareAccessSelector", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.value = "me";
    mocks.workspaces = [];
  });

  it("shows the three access modes and persists the selection", () => {
    render(<DefaultMeetingShareAccessSelector />);

    expect(
      screen.getByRole("button", { name: "Default sharing" }).textContent,
    ).toBe("me");
    expect(screen.getByText("Only me")).toBeTruthy();
    expect(screen.getByText("People in the meeting")).toBeTruthy();
    expect(screen.getByText("Everyone in the workspace")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Everyone in the workspace" }),
    ).toHaveProperty("disabled", true);

    fireEvent.click(
      screen.getByRole("button", { name: "People in the meeting" }),
    );

    expect(mocks.setValue).toHaveBeenCalledWith("participants");
  });

  it("names the workspace option and allows selecting it", () => {
    mocks.workspaces = [{ id: "workspace-1", name: "Fastrepl" }];

    render(<DefaultMeetingShareAccessSelector />);

    fireEvent.click(
      screen.getByRole("button", { name: "Everyone in Fastrepl" }),
    );

    expect(mocks.setValue).toHaveBeenCalledWith("workspace");
  });

  it("falls back to only me for unknown stored values", () => {
    mocks.value = "public";

    render(<DefaultMeetingShareAccessSelector />);

    expect(
      screen.getByRole("button", { name: "Default sharing" }).textContent,
    ).toBe("me");
  });
});
