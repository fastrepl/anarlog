import { t } from "@lingui/core/macro";
import { Sidebar, SidebarSimple } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useRef, useState } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import { LeftSurfaceChromeButton } from "./sidebar-timeline-chrome";

import { useShell } from "~/contexts/shell";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useNewNote } from "~/shared/useNewNote";
import { useSidebarUpcomingMeetingStatus } from "~/sidebar/timeline/upcoming-meeting";
import { useTabs } from "~/store/zustand/tabs";

const appWindow = getCurrentWindow();

export function WindowsTitleBar() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const openNew = useTabs((state) => state.openNew);
  const createNewNote = useNewNote();
  const upcomingMeetingStatus = useSidebarUpcomingMeetingStatus();
  const [isMaximized, setIsMaximized] = useState(false);
  const editTargetRef = useRef<HTMLElement | null>(null);
  const currentSessionId =
    currentTab?.type === "sessions" ? currentTab.id : undefined;
  const showUpcomingMeetingBadge =
    !leftsidebar.expanded &&
    !!upcomingMeetingStatus &&
    (!currentSessionId ||
      upcomingMeetingStatus.itemKey !== `session-${currentSessionId}`);

  const syncMaximized = useCallback(() => {
    void appWindow
      .isMaximized()
      .then(setIsMaximized)
      .catch(() => setIsMaximized(false));
  }, []);

  useMountEffect(() => {
    let cancelled = false;
    let unlistenResize: (() => void) | undefined;

    const sync = () => {
      if (!cancelled) {
        syncMaximized();
      }
    };

    sync();
    void appWindow
      .onResized(sync)
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlistenResize = unlisten;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlistenResize?.();
    };
  });

  const rememberEditTarget = useCallback(() => {
    editTargetRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
  }, []);
  const runEditCommand = useCallback((command: string) => {
    const editTarget = editTargetRef.current;
    if (editTarget?.isConnected) {
      editTarget.focus();
    }

    document.execCommand(command);
  }, []);
  const toggleFullscreen = useCallback(async () => {
    const isFullscreen = await appWindow.isFullscreen();
    await appWindow.setFullscreen(!isFullscreen);
  }, []);
  const toggleMaximize = useCallback(async () => {
    await appWindow.toggleMaximize();
    syncMaximized();
  }, [syncMaximized]);

  return (
    <header
      data-tauri-drag-region
      data-testid="windows-title-bar"
      className="bg-background flex h-10 shrink-0 items-stretch"
    >
      <div
        data-tauri-drag-region
        className="flex min-w-0 flex-1 items-center pl-2"
      >
        <LeftSurfaceChromeButton
          ariaLabel={leftsidebar.expanded ? t`Hide sidebar` : t`Show sidebar`}
          badge={showUpcomingMeetingBadge ? "upcomingMeeting" : null}
          onClick={leftsidebar.toggleExpanded}
        >
          {leftsidebar.expanded ? (
            <SidebarSimple size={16} />
          ) : (
            <Sidebar size={16} />
          )}
        </LeftSurfaceChromeButton>
        <nav
          aria-label={t`Application menu`}
          className="ml-2 flex h-full items-center"
          role="menubar"
        >
          <TitleBarMenu label={t`File`} onPointerDown={rememberEditTarget}>
            <DropdownMenuItem onSelect={createNewNote}>
              {t`New Note`}
              <DropdownMenuShortcut>Ctrl+N</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() =>
                openNew({ type: "settings", state: { tab: "app" } })
              }
            >
              {t`Settings`}
              <DropdownMenuShortcut>Ctrl+,</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => void appWindow.close()}>
              {t`Close`}
              <DropdownMenuShortcut>Alt+F4</DropdownMenuShortcut>
            </DropdownMenuItem>
          </TitleBarMenu>
          <TitleBarMenu label={t`Edit`} onPointerDown={rememberEditTarget}>
            <DropdownMenuItem onSelect={() => runEditCommand("undo")}>
              {t`Undo`}
              <DropdownMenuShortcut>Ctrl+Z</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => runEditCommand("redo")}>
              {t`Redo`}
              <DropdownMenuShortcut>Ctrl+Y</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => runEditCommand("cut")}>
              {t`Cut`}
              <DropdownMenuShortcut>Ctrl+X</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => runEditCommand("copy")}>
              {t`Copy`}
              <DropdownMenuShortcut>Ctrl+C</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => runEditCommand("paste")}>
              {t`Paste`}
              <DropdownMenuShortcut>Ctrl+V</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => runEditCommand("selectAll")}>
              {t`Select All`}
              <DropdownMenuShortcut>Ctrl+A</DropdownMenuShortcut>
            </DropdownMenuItem>
          </TitleBarMenu>
          <TitleBarMenu label={t`View`} onPointerDown={rememberEditTarget}>
            <DropdownMenuItem onSelect={leftsidebar.toggleExpanded}>
              {leftsidebar.expanded ? t`Hide Sidebar` : t`Show Sidebar`}
              <DropdownMenuShortcut>Ctrl+\</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void toggleFullscreen()}>
              {t`Full Screen`}
              <DropdownMenuShortcut>F11</DropdownMenuShortcut>
            </DropdownMenuItem>
          </TitleBarMenu>
          <TitleBarMenu label={t`Help`} onPointerDown={rememberEditTarget}>
            <DropdownMenuItem
              onSelect={() =>
                void openerCommands.openUrl("https://docs.anarlog.so", null)
              }
            >
              {t`Documentation`}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              onSelect={() =>
                void openerCommands.openUrl("https://anarlog.so/discord", null)
              }
            >
              {t`Report a Bug`}
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() =>
                void openerCommands.openUrl("https://anarlog.so/discord", null)
              }
            >
              {t`Suggest a Feature`}
            </DropdownMenuItem>
          </TitleBarMenu>
        </nav>
        <div data-tauri-drag-region className="min-w-4 flex-1" />
      </div>
      <div className="flex shrink-0" data-tauri-drag-region="false">
        <WindowControlButton
          ariaLabel={t`Minimize`}
          onClick={() => void appWindow.minimize()}
        >
          <span className="h-px w-2.5 bg-current" />
        </WindowControlButton>
        <WindowControlButton
          ariaLabel={isMaximized ? t`Restore` : t`Maximize`}
          onClick={() => void toggleMaximize()}
        >
          {isMaximized ? <RestoreIcon /> : <MaximizeIcon />}
        </WindowControlButton>
        <WindowControlButton
          ariaLabel={t`Close`}
          close
          onClick={() => void appWindow.close()}
        >
          <span className="relative size-3">
            <span className="absolute top-[5.5px] left-0 h-px w-3 rotate-45 bg-current" />
            <span className="absolute top-[5.5px] left-0 h-px w-3 -rotate-45 bg-current" />
          </span>
        </WindowControlButton>
      </div>
    </header>
  );
}

function TitleBarMenu({
  children,
  label,
  onPointerDown,
}: {
  children: React.ReactNode;
  label: string;
  onPointerDown: () => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          data-tauri-drag-region="false"
          role="menuitem"
          className={cn([
            "text-muted-foreground h-7 rounded-md px-2.5 text-sm",
            "hover:bg-accent hover:text-foreground data-[state=open]:bg-accent data-[state=open]:text-foreground",
            "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
          ])}
          onPointerDown={onPointerDown}
        >
          {label}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="start"
        sideOffset={1}
        className="w-56 rounded-lg"
      >
        {children}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function WindowControlButton({
  ariaLabel,
  children,
  close = false,
  onClick,
}: {
  ariaLabel: string;
  children: React.ReactNode;
  close?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      data-tauri-drag-region="false"
      className={cn([
        "text-foreground flex h-10 w-[46px] items-center justify-center transition-colors",
        close
          ? "hover:bg-[#c42b1c] hover:text-white"
          : "hover:bg-foreground/10",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden focus-visible:ring-inset",
      ])}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function MaximizeIcon() {
  return <span className="size-2.5 border border-current" />;
}

function RestoreIcon() {
  return (
    <span className="relative size-3">
      <span className="absolute top-0.5 right-0 size-2 border border-current" />
      <span className="bg-background absolute bottom-0.5 left-0 size-2 border border-current" />
    </span>
  );
}
