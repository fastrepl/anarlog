import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { useCallback } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Kbd } from "@anlg/ui/components/ui/kbd";

import { FloatingChatCTA } from "~/shared/chat-cta";
import { StandardContentWrapper } from "~/shared/main";
import { useNewNote, useNewNoteAndListen } from "~/shared/useNewNote";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function TabContentEmpty({
  tab: _tab,
}: {
  tab: Extract<Tab, { type: "empty" }>;
}) {
  return (
    <StandardContentWrapper floatingButton={<FloatingChatCTA />}>
      <EmptyView />
    </StandardContentWrapper>
  );
}

function EmptyView() {
  const newNote = useNewNote({ behavior: "current" });
  const newNoteAndListen = useNewNoteAndListen({ behavior: "current" });
  const openCurrent = useTabs((state) => state.openCurrent);
  const primaryModifier = platform() === "macos" ? "⌘" : "Ctrl";

  const openSettings = useCallback(
    () => openCurrent({ type: "settings" }),
    [openCurrent],
  );

  return (
    <div data-tauri-drag-region {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.actions)}>
        <ActionItem
          label={<Trans>New Note</Trans>}
          shortcut={[primaryModifier, "N"]}
          onClick={newNote}
        />
        <ActionItem
          label={<Trans>Start Recording</Trans>}
          shortcut={[primaryModifier, "⇧", "N"]}
          onClick={newNoteAndListen}
        />
        <div {...stylex.props(styles.separator)} />
        <ActionItem
          label={<Trans>Settings</Trans>}
          shortcut={[primaryModifier, ","]}
          onClick={openSettings}
        />
      </div>
    </div>
  );
}

function ActionItem({
  label,
  shortcut,
  icon,
  onClick,
}: {
  label: React.ReactNode;
  shortcut?: string[];
  icon?: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      data-tauri-drag-region="false"
      {...stylex.props(styles.action, stylex.defaultMarker())}
    >
      <span>{label}</span>
      {shortcut && shortcut.length > 0 ? (
        <Kbd sx={styles.shortcut}>{shortcut.join(" ")}</Kbd>
      ) : (
        icon
      )}
    </button>
  );
}

const styles = stylex.create({
  action: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: colors.foreground,
    cursor: "pointer",
    display: "flex",
    fontSize: "0.875rem",
    gap: "2rem",
    justifyContent: "space-between",
    paddingBlock: "0.5rem",
    paddingInline: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  actions: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
    minWidth: "280px",
    textAlign: "center",
  },
  root: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
    height: "100%",
    justifyContent: "center",
  },
  separator: {
    backgroundColor: colors.accent,
    height: "1px",
    marginBlock: "0.25rem",
  },
  shortcut: {
    boxShadow: {
      default: null,
      [stylex.when.ancestor(":active")]: "none",
      [stylex.when.ancestor(":hover")]:
        "0 2px 0 0 var(--kbd-shadow-outer-hover), inset 0 1px 0 0 var(--kbd-shadow-inset)",
    },
    transform: {
      default: "none",
      [stylex.when.ancestor(":active")]: "translateY(0.125rem)",
      [stylex.when.ancestor(":hover")]: "translateY(-0.125rem)",
    },
    transitionDuration: "100ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});
