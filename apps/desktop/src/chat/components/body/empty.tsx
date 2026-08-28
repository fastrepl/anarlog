import { t } from "@lingui/core/macro";
import {
  Envelope,
  ListChecks,
  MagnifyingGlass,
  Sparkle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import type { ContextRef } from "~/chat/context/entities";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useTabs } from "~/store/zustand/tabs";

export function ChatBodyEmpty({
  isModelConfigured = true,
  hasContext = false,
  onSendMessage,
}: {
  isModelConfigured?: boolean;
  hasContext?: boolean;
  onSendMessage?: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
}) {
  const { isDarkAppearance } = useChatAppearance();
  const openNew = useTabs((state) => state.openNew);
  const suggestions = [
    {
      label: t`List action items.`,
      icon: ListChecks,
      prompt: t`What are my action items from this meeting?`,
    },
    {
      label: t`Draft follow-up email.`,
      icon: Envelope,
      prompt: t`Draft a follow-up email to the participants`,
    },
    {
      label: t`Find key decisions.`,
      icon: MagnifyingGlass,
      prompt: t`What were the key decisions that have been made?`,
    },
  ];

  const handleGoToSettings = useCallback(() => {
    openNew({ type: "settings", state: { tab: "intelligence" } });
  }, [openNew]);

  const handleSuggestionClick = useCallback(
    (prompt: string) => {
      onSendMessage?.(prompt, [{ type: "text", text: prompt }]);
    },
    [onSendMessage],
  );

  if (!isModelConfigured) {
    return (
      <div {...stylex.props(styles.unconfiguredRoot)}>
        <div {...stylex.props(styles.column)}>
          <div {...stylex.props(styles.headingRow)}>
            <span
              {...stylex.props([
                styles.heading,
                isDarkAppearance ? styles.darkHeading : styles.lightHeading,
              ])}
            >
              Anarlog AI
            </span>
            <BetaChip isDarkAppearance={isDarkAppearance} />
          </div>
          <p
            {...stylex.props([
              styles.description,
              isDarkAppearance
                ? styles.darkDescription
                : styles.lightDescription,
            ])}
          >
            {t`Hi, I'm Anarlog AI. Set up a language model and I'll be ready to help.`}
          </p>
          <button
            onClick={handleGoToSettings}
            {...stylex.props(styles.settingsButton)}
          >
            <Sparkle size={12} />
            {t`Open AI Settings`}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.configuredRoot)}>
      <div {...stylex.props(styles.column)}>
        {hasContext && (
          <div {...stylex.props(styles.suggestionList)}>
            {suggestions.map(({ label, icon: Icon, prompt }) => (
              <button
                key={label}
                data-chat-suggestion
                onClick={() => handleSuggestionClick(prompt)}
                {...stylex.props([
                  styles.suggestion,
                  isDarkAppearance
                    ? styles.darkSuggestion
                    : styles.lightSuggestion,
                ])}
              >
                <span {...stylex.props(styles.suggestionIconSlot)}>
                  <Icon
                    size={16}
                    {...stylex.props([
                      styles.suggestionIcon,
                      isDarkAppearance
                        ? styles.darkSuggestionIcon
                        : styles.lightSuggestionIcon,
                    ])}
                  />
                </span>
                <span {...stylex.props(styles.suggestionLabel)}>{label}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function BetaChip({ isDarkAppearance }: { isDarkAppearance: boolean }) {
  return (
    <span
      {...stylex.props([
        styles.betaChip,
        isDarkAppearance ? styles.darkBetaChip : styles.lightBetaChip,
      ])}
    >
      {t`Beta`}
    </span>
  );
}

const styles = stylex.create({
  unconfiguredRoot: {
    display: "flex",
    justifyContent: "flex-start",
    paddingBottom: "0.25rem",
    paddingTop: "0.5rem",
  },
  configuredRoot: {
    display: "flex",
    justifyContent: "flex-start",
    paddingBottom: "0.25rem",
  },
  column: {
    display: "flex",
    flexDirection: "column",
    width: "100%",
  },
  headingRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.5rem",
  },
  heading: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  darkHeading: {
    color: colors.primaryForeground,
  },
  lightHeading: {
    color: colors.foreground,
  },
  description: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginBottom: "0.5rem",
  },
  darkDescription: {
    color: `color-mix(in oklab, ${colors.primaryForeground} 80%, transparent)`,
  },
  lightDescription: {
    color: colors.mutedForeground,
  },
  settingsButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderColor: colors.primary,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 4px 14px rgb(87 83 78 / 0.18)",
    color: colors.primaryForeground,
    display: "inline-flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.375rem",
    lineHeight: "1rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "fit-content",
  },
  suggestionList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  suggestion: {
    alignItems: "center",
    borderRadius: radii.lg,
    columnGap: "0.375rem",
    display: "grid",
    fontSize: "0.875rem",
    gridTemplateColumns: "1.5rem minmax(0, 1fr)",
    lineHeight: "1.25rem",
    paddingBottom: "0.5rem",
    paddingLeft: 0,
    paddingRight: "0.75rem",
    paddingTop: "0.5rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  darkSuggestion: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 10%, transparent)`,
    },
    color: `color-mix(in oklab, ${colors.primaryForeground} 85%, transparent)`,
  },
  lightSuggestion: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.muted} 55%, transparent)`,
    },
    color: colors.mutedForeground,
  },
  suggestionIconSlot: {
    alignItems: "center",
    display: "flex",
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  suggestionIcon: {
    flexShrink: 0,
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  darkSuggestionIcon: {
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 55%, transparent)`,
      ":is([data-chat-suggestion]:hover *)": `color-mix(in oklab, ${colors.primaryForeground} 80%, transparent)`,
    },
  },
  lightSuggestionIcon: {
    color: {
      default: `color-mix(in oklab, ${colors.mutedForeground} 75%, transparent)`,
      ":is([data-chat-suggestion]:hover *)": colors.foreground,
    },
  },
  suggestionLabel: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  betaChip: {
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    fontSize: "0.625rem",
    fontWeight: 500,
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  darkBetaChip: {
    backgroundColor: colors.accent,
    borderColor: colors.border,
    color: colors.accentForeground,
  },
  lightBetaChip: {
    backgroundColor: "oklch(95.1% 0.026 236.824)",
    borderColor: "oklch(90.1% 0.058 230.902)",
    color: "oklch(39.1% 0.09 240.876)",
  },
});
