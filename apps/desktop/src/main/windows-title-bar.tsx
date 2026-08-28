import { t } from "@lingui/core/macro";
import { Sidebar, SidebarSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

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
      {...stylex.props(styles.header)}
    >
      <div data-tauri-drag-region {...stylex.props(styles.leading)}>
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
          {...stylex.props(styles.menu)}
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
        <div data-tauri-drag-region {...stylex.props(styles.dragSpacer)} />
      </div>
      <div
        {...stylex.props(styles.windowControls)}
        data-tauri-drag-region="false"
      >
        <WindowControlButton
          ariaLabel={t`Minimize`}
          onClick={() => void appWindow.minimize()}
        >
          <span {...stylex.props(styles.minimizeIcon)} />
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
          <span {...stylex.props(styles.closeIcon)}>
            <span
              {...stylex.props(styles.closeLine, styles.closeLineForward)}
            />
            <span
              {...stylex.props(styles.closeLine, styles.closeLineBackward)}
            />
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
          {...stylex.props(styles.titleBarMenu)}
          onPointerDown={onPointerDown}
        >
          {label}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" sideOffset={1} sx={styles.menuContent}>
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
      {...stylex.props(
        styles.windowControlButton,
        close ? styles.closeButton : styles.standardButton,
      )}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function MaximizeIcon() {
  return <span {...stylex.props(styles.maximizeIcon)} />;
}

function RestoreIcon() {
  return (
    <span {...stylex.props(styles.restoreIcon)}>
      <span {...stylex.props(styles.restoreBack)} />
      <span {...stylex.props(styles.restoreFront)} />
    </span>
  );
}

const styles = stylex.create({
  closeButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "#c42b1c",
    },
    color: {
      default: colors.foreground,
      ":hover": "white",
    },
  },
  closeIcon: {
    height: "0.75rem",
    position: "relative",
    width: "0.75rem",
  },
  closeLine: {
    backgroundColor: "currentColor",
    height: "1px",
    left: 0,
    position: "absolute",
    top: "5.5px",
    width: "0.75rem",
  },
  closeLineBackward: {
    transform: "rotate(-45deg)",
  },
  closeLineForward: {
    transform: "rotate(45deg)",
  },
  dragSpacer: {
    flex: "1",
    minWidth: "1rem",
  },
  header: {
    alignItems: "stretch",
    backgroundColor: colors.background,
    borderBottomColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    flexShrink: 0,
    height: "2.5rem",
  },
  leading: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    minWidth: 0,
    paddingLeft: "0.5rem",
  },
  maximizeIcon: {
    borderColor: "currentColor",
    borderStyle: "solid",
    borderWidth: "1px",
    height: "0.625rem",
    width: "0.625rem",
  },
  menu: {
    alignItems: "center",
    display: "flex",
    height: "100%",
    marginLeft: "0.5rem",
  },
  menuContent: {
    borderRadius: radii.lg,
    width: "14rem",
  },
  minimizeIcon: {
    backgroundColor: "currentColor",
    height: "1px",
    width: "0.625rem",
  },
  restoreBack: {
    borderColor: "currentColor",
    borderStyle: "solid",
    borderWidth: "1px",
    height: "0.5rem",
    position: "absolute",
    right: 0,
    top: "0.125rem",
    width: "0.5rem",
  },
  restoreFront: {
    backgroundColor: colors.background,
    borderColor: "currentColor",
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: "0.125rem",
    height: "0.5rem",
    left: 0,
    position: "absolute",
    width: "0.5rem",
  },
  restoreIcon: {
    height: "0.75rem",
    position: "relative",
    width: "0.75rem",
  },
  standardButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.foreground} 10%, transparent)`,
    },
  },
  titleBarMenu: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
      ':is([data-state="open"])': colors.accent,
    },
    borderRadius: radii.md,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
      ':is([data-state="open"])': colors.foreground,
    },
    fontSize: "0.875rem",
    height: "1.75rem",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    paddingInline: "0.625rem",
  },
  windowControlButton: {
    alignItems: "center",
    boxShadow: {
      default: null,
      ":focus-visible": `inset 0 0 0 2px ${colors.ring}`,
    },
    display: "flex",
    height: "2.5rem",
    justifyContent: "center",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "46px",
  },
  windowControls: {
    display: "flex",
    flexShrink: 0,
  },
});
