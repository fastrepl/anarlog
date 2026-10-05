import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AlwaysOnTop } from "./always-on-top";

const { isAlwaysOnTopMock, setAlwaysOnTopMock } = vi.hoisted(() => ({
  isAlwaysOnTopMock: vi.fn(() => Promise.resolve(false)),
  setAlwaysOnTopMock: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isAlwaysOnTop: isAlwaysOnTopMock,
    setAlwaysOnTop: setAlwaysOnTopMock,
  }),
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  DropdownMenuItem: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
}));

async function enabledItem() {
  const item = await screen.findByRole("button", { name: "Always on Top" });
  await vi.waitFor(() =>
    expect((item as HTMLButtonElement).disabled).toBe(false),
  );
  return item;
}

describe("AlwaysOnTop", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("keeps the pinned state when isAlwaysOnTop misreports", async () => {
    const first = render(<AlwaysOnTop />);
    fireEvent.click(await enabledItem());
    expect(setAlwaysOnTopMock).toHaveBeenCalledWith(true);
    first.unmount();

    // Reopening the menu must reflect the pinned state even though
    // isAlwaysOnTop still resolves false (Linux misreport).
    const second = render(<AlwaysOnTop />);
    const item = await enabledItem();
    expect(second.container.querySelector(".ml-auto")).not.toBeNull();

    fireEvent.click(item);
    expect(setAlwaysOnTopMock).toHaveBeenCalledWith(false);
  });
});
