import { Trans, useLingui } from "@lingui/react/macro";
import { DotsThree, MinusCircle, PushPin, Trash } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

export function ContactPageHeader({
  title,
  compactIdentity,
  showCompactIdentity,
  pinned,
  onTogglePin,
  onDelete,
  onRemoveAvatar,
}: {
  title: string;
  compactIdentity: ReactNode;
  showCompactIdentity: boolean;
  pinned: boolean;
  onTogglePin: () => void;
  onDelete: () => void;
  onRemoveAvatar?: () => void;
}) {
  const { t } = useLingui();

  return (
    <div data-tauri-drag-region {...stylex.props(styles.header)}>
      <div {...stylex.props(styles.identity)}>
        {showCompactIdentity && compactIdentity}
        <h2 {...stylex.props(styles.title)}>{title}</h2>
      </div>
      <div data-tauri-drag-region="false" {...stylex.props(styles.actions)}>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              size="icon"
              variant="ghost"
              data-tauri-drag-region="false"
              sx={styles.optionsButton}
              aria-label={t`Contact options`}
            >
              <DotsThree size={16} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent variant="app" align="end" sx={styles.menu}>
            <AppFloatingPanel sx={styles.menuPanel}>
              <DropdownMenuItem onClick={onTogglePin} sx={styles.menuItem}>
                <PushPin weight={pinned ? "fill" : "regular"} />
                <span>
                  {pinned ? <Trans>Unpin</Trans> : <Trans>Pin</Trans>}
                </span>
              </DropdownMenuItem>
              {onRemoveAvatar && (
                <DropdownMenuItem onClick={onRemoveAvatar} sx={styles.menuItem}>
                  <MinusCircle />
                  <span>
                    <Trans>Remove photo</Trans>
                  </span>
                </DropdownMenuItem>
              )}
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={onDelete}
                sx={[styles.menuItem, styles.deleteItem]}
              >
                <Trash />
                <span>
                  <Trans>Delete</Trans>
                </span>
              </DropdownMenuItem>
            </AppFloatingPanel>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
  },
  deleteItem: {
    backgroundColor: {
      default: "transparent",
      ":hover": "rgb(254 242 242)",
      ":is(.dark *):hover": "rgb(69 10 10 / 0.5)",
    },
    color: {
      default: "rgb(220 38 38)",
      ":hover": "rgb(185 28 28)",
      ":is(.dark *)": "rgb(248 113 113)",
      ":is(.dark *):hover": "rgb(252 165 165)",
    },
  },
  header: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.75rem",
    height: "3rem",
    justifyContent: "space-between",
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
  },
  identity: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.5rem",
    minWidth: 0,
  },
  menu: {
    width: "12rem",
  },
  menuItem: {
    cursor: "pointer",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  optionsButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
