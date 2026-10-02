import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { Fragment, StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useChatContext } from "~/chat/state/chat-context";
import { ShellProvider, useShell } from "~/contexts/shell";
import { CustomSidebarHeader } from "~/sidebar/custom-sidebar-header";
import { useTabs } from "~/store/zustand/tabs";

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("./body", async () => {
  const { ClassicMainSidebar } = await import("./shell-sidebar");
  return { ClassicMainBody: ClassicMainSidebar };
});

vi.mock("./windows-title-bar", () => ({ WindowsTitleBar: () => null }));
vi.mock("~/devtools-bar", () => ({ DevtoolsStatusBar: () => null }));
vi.mock("~/sidebar/toast", () => ({ ToastNotifications: () => null }));

vi.mock("~/shared/main", () => ({
  MainShellScaffold: ({ children }: { children: React.ReactNode }) => children,
  MainShellBodyFrame: ({ children }: { children: React.ReactNode }) => {
    const { chat } = useShell();
    return <Fragment key={chat.sessionId}>{children}</Fragment>;
  },
}));

vi.mock("~/sidebar", () => ({
  LeftSidebar: () => {
    const currentTab = useTabs((state) => state.currentTab);
    return currentTab?.type === "empty" ? (
      <div>Notes sidebar</div>
    ) : (
      <CustomSidebarHeader />
    );
  },
}));

import { ClassicMainShellFrame } from "./shell-frame";

function SidebarToggle() {
  const { leftsidebar } = useShell();
  return (
    <button onClick={leftsidebar.toggleExpanded}>
      {leftsidebar.expanded ? "Hide sidebar" : "Show sidebar"}
    </button>
  );
}

describe("custom sidebar chat lifecycle", () => {
  beforeEach(() => {
    useTabs.setState(useTabs.getInitialState());
    useChatContext.setState(useChatContext.getInitialState());
    useTabs.getState().openCurrent({ type: "empty" });
  });

  afterEach(cleanup);

  it.each([
    { expanded: false, strict: true },
    { expanded: true, strict: true },
  ])(
    "keeps Automations navigation usable across chat changes (expanded=$expanded, strict=$strict)",
    ({ expanded, strict }) => {
      const Wrapper = strict ? StrictMode : Fragment;
      render(
        <Wrapper>
          <ShellProvider>
            <SidebarToggle />
            <ClassicMainShellFrame />
          </ShellProvider>
        </Wrapper>,
      );

      if (!expanded) {
        fireEvent.click(screen.getByRole("button", { name: "Hide sidebar" }));
      }

      act(() => useTabs.getState().openNew({ type: "settings" }));
      expect(screen.getByRole("button", { name: "Go home" })).toBeTruthy();

      act(() => useTabs.getState().openNew({ type: "automations" }));
      expect(screen.getByRole("button", { name: "Go home" })).toBeTruthy();

      act(() => useChatContext.getState().startNewChat("automations"));
      expect(screen.getByRole("button", { name: "Go home" })).toBeTruthy();

      act(() =>
        useChatContext.getState().selectChat("automations", "saved-chat"),
      );
      expect(screen.getByRole("button", { name: "Go home" })).toBeTruthy();

      fireEvent.click(screen.getByRole("button", { name: "Hide sidebar" }));
      expect(screen.getByRole("button", { name: "Go home" })).toBeTruthy();

      fireEvent.click(screen.getByRole("button", { name: "Go home" }));
      expect(useTabs.getState().currentTab?.type).toBe("settings");
      fireEvent.click(screen.getByRole("button", { name: "Go home" }));
      expect(useTabs.getState().currentTab?.type).toBe("empty");
      expect(screen.queryByText("Notes sidebar") !== null).toBe(expanded);

      fireEvent.click(
        screen.getByRole("button", {
          name: expanded ? "Hide sidebar" : "Show sidebar",
        }),
      );
      expect(screen.queryByText("Notes sidebar") !== null).toBe(!expanded);
    },
  );
});
