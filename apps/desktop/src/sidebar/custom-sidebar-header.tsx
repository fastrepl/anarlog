import { useLingui } from "@lingui/react/macro";
import { ArrowLeft } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type ReactNode, useCallback } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { useShell } from "~/contexts/shell";
import { useWindowControlsGutter } from "~/shared/hooks/useWindowControlsGutter";
import { useTabs } from "~/store/zustand/tabs";

export function CustomSidebarHeader({ children }: { children?: ReactNode }) {
  const { t } = useLingui();
  const { chat } = useShell();
  const showWindowControlsGutter = useWindowControlsGutter();
  const currentTab = useTabs((state) => state.currentTab);
  const tabs = useTabs((state) => state.tabs);
  const select = useTabs((state) => state.select);
  const openCurrent = useTabs((state) => state.openCurrent);

  const handleBack = useCallback(() => {
    if (currentTab?.type !== "automations" && chat.mode !== "FloatingClosed") {
      chat.sendEvent({ type: "CLOSE" });
      return;
    }

    if (currentTab?.type === "onboarding" || currentTab?.type === "empty") {
      return;
    }

    const existingHomeTab = tabs.find((tab) => tab.type === "empty");
    if (existingHomeTab) {
      select(existingHomeTab);
      return;
    }

    openCurrent({ type: "empty" });
  }, [chat, currentTab, openCurrent, select, tabs]);

  return (
    <div
      data-tauri-drag-region
      {...stylex.props(
        styles.header,
        showWindowControlsGutter
          ? styles.headerWithWindowControls
          : styles.headerDefault,
      )}
    >
      <div data-tauri-drag-region {...stylex.props(styles.leading)}>
        <CustomSidebarHeaderButton
          label={t`Go home`}
          title={t`Back`}
          onClick={handleBack}
        >
          <ArrowLeft size={16} />
        </CustomSidebarHeaderButton>
      </div>
      {children ? (
        <div data-tauri-drag-region="false" {...stylex.props(styles.trailing)}>
          {children}
        </div>
      ) : null}
    </div>
  );
}

function CustomSidebarHeaderButton({
  children,
  disabled = false,
  label,
  onClick,
  title,
}: {
  children: ReactNode;
  disabled?: boolean;
  label: string;
  onClick: () => void;
  title?: string;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={title}
      data-tauri-drag-region="false"
      disabled={disabled}
      {...stylex.props(styles.button)}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

const styles = stylex.create({
  button: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
      ":disabled:hover": "transparent",
    },
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
      ":disabled": `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
      ":disabled:hover": `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    },
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
    zIndex: 50,
  },
  header: {
    alignItems: "flex-start",
    display: "flex",
    flexShrink: 0,
    height: "3rem",
    paddingBottom: 0,
    paddingRight: "0.25rem",
    paddingTop: "9px",
  },
  headerDefault: {
    paddingLeft: "0.5rem",
  },
  headerWithWindowControls: {
    paddingLeft: "76px",
  },
  leading: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.25rem",
    minWidth: 0,
  },
  trailing: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    marginLeft: "0.25rem",
  },
});
