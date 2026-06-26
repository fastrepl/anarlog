import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { openNewMock } = vi.hoisted(() => ({
  openNewMock: vi.fn(),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: <T,>(selector: (state: { openNew: typeof openNewMock }) => T) =>
    selector({ openNew: openNewMock }),
}));

import { ContextBar } from "./context-bar";

describe("ContextBar", () => {
  beforeEach(() => {
    cleanup();
    openNewMock.mockClear();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  });

  it("renders context as chips without an add session control", () => {
    render(
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:auto:current",
            source: "auto-current",
            sessionId: "current",
            title: "Current Note",
            date: "2026-04-01T00:00:00.000Z",
            pending: true,
          },
        ]}
      />,
    );

    expect(screen.getAllByText("Current Note").length).toBeGreaterThan(0);
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByText("Search sessions...")).toBeNull();
  });

  it("does not render chip tooltips", () => {
    render(
      <ContextBar
        entities={[
          {
            kind: "human",
            key: "human:manual:artem",
            source: "manual",
            humanId: "artem",
            name: "Artem",
            email: "artem@example.com",
            organizationName: "Char",
            pending: true,
          },
        ]}
      />,
    );

    expect(screen.getByText("Artem")).toBeTruthy();
    expect(screen.queryByText(/artem@example.com/)).toBeNull();
  });

  it("centers the squircle chip strip above the input surface", () => {
    const { container } = render(
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:auto:current",
            source: "auto-current",
            sessionId: "current",
            title: "Current Note",
            date: "2026-04-01T00:00:00.000Z",
            pending: false,
          },
        ]}
      />,
    );

    const outer = document.querySelector("[data-chat-context-bar]");
    const chipList = container.querySelector("[data-chat-context-chip-list]");
    const chipRow = chipList?.parentElement;
    const chip = container.querySelector("[data-chat-context-chip]");

    expect(outer?.className).toContain("px-3");
    expect(outer?.className).toContain("pb-1.5");
    expect(outer?.className).not.toContain("border");
    expect(outer?.className).not.toContain("rounded-t-xl");
    expect(chipRow?.className).toContain("justify-center");
    expect(chip?.className).toContain("rounded-[10px]");
  });

  it("shows four chips and expands hidden chips from the overflow control", () => {
    const { container } = render(
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:manual:first",
            source: "manual",
            sessionId: "first",
            title: "First Note",
            date: "2026-04-01T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:second",
            source: "manual",
            sessionId: "second",
            title: "Second Note",
            date: "2026-04-02T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:third",
            source: "manual",
            sessionId: "third",
            title: "Third Note",
            date: "2026-04-03T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:fourth",
            source: "manual",
            sessionId: "fourth",
            title: "Fourth Note",
            date: "2026-04-04T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:fifth",
            source: "manual",
            sessionId: "fifth",
            title: "Fifth Note",
            date: "2026-04-05T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:sixth",
            source: "manual",
            sessionId: "sixth",
            title: "Sixth Note",
            date: "2026-04-06T00:00:00.000Z",
            pending: true,
          },
        ]}
      />,
    );

    const chipList = container.querySelector("[data-chat-context-chip-list]");

    expect(chipList?.className).toContain("overflow-hidden");
    expect(chipList?.className).not.toContain("flex-wrap");
    expect(container.querySelectorAll("[data-chat-context-chip]")).toHaveLength(
      4,
    );
    expect(screen.queryByText("Fifth Note")).toBeNull();
    expect(screen.queryByText("Sixth Note")).toBeNull();

    fireEvent.click(
      screen.getByRole("button", {
        name: "+2 more",
      }),
    );

    expect(screen.getAllByText("Fifth Note").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Sixth Note").length).toBeGreaterThan(0);
    expect(container.querySelectorAll("[data-chat-context-chip]")).toHaveLength(
      6,
    );
    expect(chipList?.className).toContain("flex-wrap");
    expect(screen.queryByText("+2 more")).toBeNull();

    fireEvent.click(
      screen.getByRole("button", {
        name: "Collapse context chips",
      }),
    );

    expect(container.querySelectorAll("[data-chat-context-chip]")).toHaveLength(
      4,
    );
    expect(screen.getByText("+2 more")).not.toBeNull();
    expect(chipList?.className).toContain("overflow-hidden");
  });

  it("wraps four-or-fewer chips instead of clipping them", () => {
    const { container } = render(
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:manual:first",
            source: "manual",
            sessionId: "first",
            title: "First Note",
            date: "2026-04-01T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:second",
            source: "manual",
            sessionId: "second",
            title: "Second Note",
            date: "2026-04-02T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:third",
            source: "manual",
            sessionId: "third",
            title: "Third Note",
            date: "2026-04-03T00:00:00.000Z",
            pending: true,
          },
          {
            kind: "session",
            key: "session:manual:fourth",
            source: "manual",
            sessionId: "fourth",
            title: "Fourth Note",
            date: "2026-04-04T00:00:00.000Z",
            pending: true,
          },
        ]}
      />,
    );

    const chipList = container.querySelector("[data-chat-context-chip-list]");
    const chip = container.querySelector("[data-chat-context-chip]");

    expect(chipList?.className).toContain("flex-wrap");
    expect(chipList?.className).not.toContain("overflow-hidden");
    expect(chip?.className).toContain("shrink-0");
    expect(screen.queryByRole("button", { name: /more/ })).toBeNull();
  });

  it("opens a session chip when clicked", () => {
    render(
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:auto:current",
            source: "auto-current",
            sessionId: "current",
            title: "Current Note",
            date: "2026-04-01T00:00:00.000Z",
            pending: false,
          },
        ]}
      />,
    );

    fireEvent.click(screen.getAllByText("Current Note")[0]);

    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "current",
    });
  });

  it("removes manual context chips", () => {
    const onRemoveEntity = vi.fn();
    const { container } = render(
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:manual:current",
            source: "manual",
            sessionId: "current",
            title: "Current Note",
            date: "2026-04-01T00:00:00.000Z",
            removable: true,
            pending: true,
          },
        ]}
        onRemoveEntity={onRemoveEntity}
      />,
    );

    const chip = container.querySelector("[data-chat-context-chip]");
    const iconSlot = chip?.firstElementChild;
    const contextIcon = iconSlot?.querySelector("svg");
    const removeButton = screen.getByRole("button", {
      name: "Remove Current Note",
    });
    const removeIcon = removeButton.querySelector("svg");

    expect(iconSlot?.className).toContain("size-4");
    expect(iconSlot?.contains(removeButton)).toBe(true);
    expect(contextIcon?.className.baseVal).toContain("group-hover:opacity-0");
    expect(removeIcon?.className.baseVal).toContain("size-3.5");
    expect(removeButton.className).toContain("group-hover:opacity-100");
    expect(removeButton.className).toContain("group-hover:pointer-events-auto");

    fireEvent.click(removeButton);

    expect(onRemoveEntity).toHaveBeenCalledWith("session:manual:current");
    expect(openNewMock).not.toHaveBeenCalled();
  });
});
