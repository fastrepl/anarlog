import { cleanup, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: "windows",
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@tanstack/react-query", () => ({
  useMutation: () => ({
    mutate: vi.fn(),
    isPending: false,
  }),
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  DropdownMenuItem: ({ children, ...props }: ComponentProps<"button">) => (
    <button {...props}>{children}</button>
  ),
}));

import { ShowInFolder } from "./misc";

describe("ShowInFolder", () => {
  afterEach(cleanup);

  it("uses platform-neutral folder copy and iconography", () => {
    mocks.platform = "windows";
    render(<ShowInFolder sessionId="session-1" />);

    const action = screen.getByRole("button", { name: "Show in folder" });
    expect(action.querySelector(".lucide-folder-open")).toBeTruthy();
    expect(screen.queryByText("Show in Finder")).toBeNull();
  });

  it("uses the native Finder term on macOS", () => {
    mocks.platform = "macos";
    render(<ShowInFolder sessionId="session-1" />);

    expect(screen.getByRole("button", { name: "Show in Finder" })).toBeTruthy();
  });
});
