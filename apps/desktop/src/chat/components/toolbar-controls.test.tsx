import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    className,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button className={className} type="button" {...props}>
      {children}
    </button>
  ),
}));

vi.mock("@hypr/ui/components/ui/dropdown-menu", () => ({
  AppFloatingPanel: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenu: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: () => undefined,
    useRow: () => undefined,
    useSortedRowIds: () => [],
  },
}));

import { ChatToolbarControls } from "./toolbar-controls";

describe("ChatToolbarControls", () => {
  beforeEach(() => {
    cleanup();
  });

  it("renders the dark chat title trigger as a pill", () => {
    render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="dark"
      />,
    );

    const title = screen.getByText("Ask Anarlog AI anything");
    expect(title.closest("button")?.className).toContain("rounded-full");
  });

  it("keeps the light chat title readable on the card shell", () => {
    const { container } = render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const title = screen.getByText("Ask Anarlog AI anything");
    const titleButton = title.closest("button");
    expect(container.firstElementChild?.className).toContain("pl-2");
    expect(container.firstElementChild?.className).toContain("pr-2");
    expect(titleButton?.className).toContain("-ml-2");
    expect(titleButton?.className).toContain("px-2");
    expect(title.className).toContain("text-foreground");
    expect(title.className).toContain("text-[15px]");
    expect(title.className).not.toContain("text-muted-foreground");
  });

  it("renders dark toolbar action buttons as circles without tooltips", () => {
    render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="dark"
      />,
    );

    const newChatButton = screen.getByRole("button", { name: "New chat" });
    const rightPanelButton = screen.getByRole("button", {
      name: "Open in right panel",
    });

    expect(newChatButton.className).toContain("rounded-full");
    expect(newChatButton.className).toContain("hover:!bg-primary-foreground/7");
    expect(newChatButton.getAttribute("title")).toBeNull();
    expect(rightPanelButton.className).toContain("rounded-full");
    expect(rightPanelButton.className).toContain(
      "hover:!bg-primary-foreground/7",
    );
    expect(rightPanelButton.getAttribute("title")).toBeNull();
  });

  it("uses tighter toolbar right padding in the right panel", () => {
    const onClose = vi.fn();
    const onOpenFloating = vi.fn();
    const { container } = render(
      <ChatToolbarControls
        currentChatGroupId={undefined}
        layout="right-panel"
        onClose={onClose}
        onNewChat={vi.fn()}
        onOpenFloating={onOpenFloating}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const titleButton = screen
      .getByText("Ask Anarlog AI anything")
      .closest("button");

    expect(container.firstElementChild?.className).toContain("pl-3");
    expect(container.firstElementChild?.className).toContain("pr-1");
    expect(container.firstElementChild?.className).not.toContain("px-5");
    expect(container.firstElementChild?.className).not.toContain("px-2");
    expect(container.firstElementChild?.className).not.toContain("pr-0");
    expect(titleButton?.className).toContain("-ml-2");
    expect(titleButton?.className).toContain("px-2");
    const floatButton = screen.getByRole("button", { name: "Float chat" });
    const closeButton = screen.getByRole("button", { name: "Close chat" });
    expect(floatButton.className).not.toContain("bg-muted");
    expect(floatButton.className).not.toContain("text-foreground");
    expect(floatButton.className).not.toContain("mr-1");
    expect(closeButton.className).not.toContain("bg-muted");
    expect(
      screen.queryByRole("button", { name: "Open in right panel" }),
    ).toBeNull();

    fireEvent.click(floatButton);
    fireEvent.click(closeButton);

    expect(onOpenFloating).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
