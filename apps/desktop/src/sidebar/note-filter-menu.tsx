import { Trans, useLingui } from "@lingui/react/macro";
import { FunnelSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import type { SidebarNoteFilter } from "./note-filter";

export function SidebarNoteFilterMenu({
  value,
  onValueChange,
}: {
  value: SidebarNoteFilter;
  onValueChange: (value: SidebarNoteFilter) => void;
}) {
  const { t } = useLingui();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t`Filter notes`}
          title={t`Filter notes`}
          data-tauri-drag-region="false"
          {...stylex.props(
            styles.trigger,
            value !== "mine" && styles.triggerActive,
          )}
        >
          <FunnelSimple
            size={15}
            weight={value === "mine" ? "regular" : "fill"}
          />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="start" sx={styles.content}>
        <AppFloatingPanel sx={styles.panel}>
          <DropdownMenuRadioGroup
            value={value}
            onValueChange={(nextValue) =>
              onValueChange(nextValue as SidebarNoteFilter)
            }
          >
            <DropdownMenuRadioItem value="mine">
              <Trans>My notes</Trans>
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="shared">
              <Trans>Shared</Trans>
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

const styles = stylex.create({
  content: {
    width: "13rem",
  },
  panel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  trigger: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
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
    pointerEvents: "auto",
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
  },
  triggerActive: {
    backgroundColor: colors.accent,
    color: colors.foreground,
  },
});
