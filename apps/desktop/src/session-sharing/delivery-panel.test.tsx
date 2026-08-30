import { fireEvent, render, screen } from "@testing-library/react";
import {
  createElement,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { describe, expect, it, vi } from "vitest";

import { ShareRecapOverflowMenu } from "./delivery-panel";

vi.mock("@iconify-icon/react", () => ({
  Icon: (props: Record<string, unknown>) =>
    createElement("iconify-icon", props),
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  appFloatingMenuPanelClassName: "overflow-hidden p-1.5",
  AppFloatingPanel: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenu: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuItem: ({
    children,
    onSelect,
    ...props
  }: ButtonHTMLAttributes<HTMLButtonElement> & { onSelect?: () => void }) => (
    <button type="button" {...props} onClick={onSelect}>
      {children}
    </button>
  ),
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
}));

describe("ShareRecapOverflowMenu", () => {
  it("offers email and Slack delivery from the overflow menu", () => {
    const onValueChange = vi.fn();
    render(<ShareRecapOverflowMenu onValueChange={onValueChange} />);

    expect(screen.queryByRole("button", { name: "People" })).toBeNull();
    expect(screen.getByRole("button", { name: "More options" })).toBeTruthy();
    expect(
      document.querySelector('iconify-icon[icon="logos:slack-icon"]'),
    ).not.toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Email" }));
    fireEvent.click(screen.getByRole("button", { name: "Slack" }));

    expect(onValueChange).toHaveBeenNthCalledWith(1, "email");
    expect(onValueChange).toHaveBeenNthCalledWith(2, "slack");
  });
});
