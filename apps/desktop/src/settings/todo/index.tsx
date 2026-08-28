import { Trans } from "@lingui/react/macro";
import { CaretDown } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@anlg/ui/components/ui/accordion";

import { TodoProviderContent } from "./provider-content";
import { TODO_PROVIDERS } from "./shared";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsTodo() {
  const isMacos = platform() === "macos";
  const visibleProviders = TODO_PROVIDERS.filter(
    (provider) =>
      provider.platform === undefined || provider.platform === "all" || isMacos,
  );

  return (
    <div {...stylex.props(styles.page)}>
      <SettingsPageTitle title={<Trans>Ticket</Trans>} />
      <Accordion type="multiple">
        {visibleProviders.map((provider) => (
          <AccordionItem key={provider.id} value={provider.id} sx={styles.item}>
            <div {...stylex.props(styles.providerRow, stylex.defaultMarker())}>
              <AccordionHeader {...stylex.props(styles.header)}>
                <AccordionTriggerPrimitive {...stylex.props(styles.trigger)}>
                  {provider.icon}
                  <span>{provider.displayName}</span>
                </AccordionTriggerPrimitive>
              </AccordionHeader>
              <CaretDown
                data-todo-provider-caret
                {...stylex.props(styles.caret)}
              />
            </div>
            <AccordionContent sx={styles.content}>
              <div {...stylex.props(styles.contentInner)}>
                <TodoProviderContent config={provider} />
              </div>
            </AccordionContent>
          </AccordionItem>
        ))}
      </Accordion>
    </div>
  );
}

const styles = stylex.create({
  caret: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    opacity: {
      default: 0,
      [stylex.when.ancestor(":focus-within")]: 1,
      [stylex.when.ancestor(":hover")]: 1,
    },
    transitionDuration: "200ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  content: {
    paddingBottom: "0.75rem",
  },
  contentInner: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  header: {
    minWidth: 0,
  },
  item: {
    borderBottomColor: colors.border,
    borderBottomStyle: {
      default: "solid",
      ":last-child": "none",
    },
    borderBottomWidth: {
      default: "1px",
      ":last-child": 0,
    },
    transform: {
      default: null,
      ':is([data-state="open"]) [data-todo-provider-caret]': "rotate(180deg)",
    },
  },
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
  },
  providerRow: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    display: "grid",
    gap: "0.25rem",
    gridTemplateColumns: "minmax(0, 1fr) auto",
  },
  trigger: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.5rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    paddingBlock: "0.75rem",
    textAlign: "left",
    textDecorationLine: {
      default: "none",
      ":hover": "none",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
});
