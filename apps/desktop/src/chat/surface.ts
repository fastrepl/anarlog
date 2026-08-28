import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

export type ChatToolbarSurface = "light" | "dark";

export function isChatDarkAppearance(): boolean {
  return false;
}

export function chatToolbarSurface(): ChatToolbarSurface {
  return "light";
}

const styles = stylex.create({
  panel: {
    backgroundColor: colors.card,
    color: colors.cardForeground,
  },
  floatingPanel: {
    backgroundColor: {
      default: "#f4f4f5",
      ":is(.dark *)": "#202020",
    },
    color: colors.cardForeground,
  },
  panelBorder: {
    borderColor: colors.border,
  },
  floatingPanelShell: {
    backgroundColor: {
      default: "#f4f4f5",
      ":is(.dark *)": "#202020",
    },
    borderBottomColor: {
      default: `color-mix(in oklab, ${colors.border} 70%, transparent)`,
      ":is(.dark *)": "rgb(255 255 255 / 0.1)",
    },
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderLeftColor: {
      default: `color-mix(in oklab, ${colors.border} 70%, transparent)`,
      ":is(.dark *)": "rgb(255 255 255 / 0.1)",
    },
    borderLeftStyle: "solid",
    borderLeftWidth: "1px",
    borderRadius: "24px",
    borderRightColor: {
      default: `color-mix(in oklab, ${colors.border} 70%, transparent)`,
      ":is(.dark *)": "rgb(255 255 255 / 0.1)",
    },
    borderRightStyle: "solid",
    borderRightWidth: "1px",
    borderTopColor: colors.appFloatingBorder,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    boxShadow: {
      default: "0 32px 84px rgb(0 0 0 / 0.32)",
      ":is(.dark *)": "0 36px 96px rgb(0 0 0 / 0.72)",
    },
    color: colors.cardForeground,
  },
  elevatedSurface: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    color: colors.cardForeground,
  },
  inputEditor: {
    color: colors.cardForeground,
  },
  sendButtonDisabled: {
    borderColor: colors.border,
    color: `color-mix(in oklab, ${colors.mutedForeground} 60%, transparent)`,
    cursor: "default",
  },
  sendButtonShortcutDisabled: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 60%, transparent)`,
  },
  floatingControl: {
    backgroundColor: {
      default: colors.accent,
      ":hover": `color-mix(in oklab, ${colors.accent} 90%, transparent)`,
    },
    borderColor: colors.border,
    color: colors.accentForeground,
  },
});

export const chatPanelStyle = styles.panel;
export const chatFloatingPanelStyle = styles.floatingPanel;
export const chatPanelBorderStyle = styles.panelBorder;
export const chatFloatingPanelShellStyle = styles.floatingPanelShell;
export const chatElevatedSurfaceStyle = styles.elevatedSurface;
export const chatInputEditorStyle = styles.inputEditor;
export const chatSendButtonDisabledStyle = styles.sendButtonDisabled;
export const chatSendButtonShortcutDisabledStyle =
  styles.sendButtonShortcutDisabled;
export const chatFloatingControlStyle = styles.floatingControl;
