import { useLingui } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { chatCta } from "./chat-cta.stylex";

import { useShell } from "~/contexts/shell";

export function ChatCTA({
  label,
  ariaLabel,
}: {
  label?: ReactNode;
  ariaLabel?: string;
}) {
  const { t } = useLingui();
  const { chat } = useShell();
  const isChatOpen = chat.mode !== "FloatingClosed";
  const resolvedLabel = label ?? t`Ask anything`;

  const handleClick = () => {
    chat.sendEvent({ type: "OPEN" });
  };

  if (isChatOpen) {
    return null;
  }

  return (
    <button
      type="button"
      data-chat-cta-trigger
      aria-label={ariaLabel ?? t`Ask Anarlog anything`}
      onClick={handleClick}
      {...stylex.props(styles.trigger)}
    >
      <span
        data-chat-cta-surface
        aria-hidden="true"
        {...stylex.props(styles.surface)}
      >
        <span aria-hidden="true" {...stylex.props(styles.label)}>
          {resolvedLabel}
        </span>
      </span>
    </button>
  );
}

export function FloatingChatCTA({ label }: { label?: ReactNode }) {
  return (
    <div {...stylex.props(styles.floating)}>
      <div {...stylex.props(styles.floatingContent)}>
        <ChatCTA label={label} />
      </div>
    </div>
  );
}

const expandedLightShadow = "0 16px 42px rgba(0, 0, 0, 0.26)";
const expandedDarkShadow = "0 18px 52px rgba(0, 0, 0, 0.64)";

const styles = stylex.create({
  floating: {
    alignItems: "flex-end",
    bottom: "0.75rem",
    display: "flex",
    height: "2.5rem",
    justifyContent: "center",
    left: "50%",
    maxWidth: "calc(100% - 2rem)",
    paddingBottom: 0,
    pointerEvents: "none",
    position: "absolute",
    transform: "translateX(-50%)",
    width: "180px",
    zIndex: 20,
  },
  floatingContent: {
    maxWidth: "100%",
    pointerEvents: "auto",
  },
  label: {
    color: chatCta.labelColor,
    flex: "1",
    minWidth: 0,
    opacity: chatCta.labelOpacity,
    overflow: "hidden",
    textAlign: "left",
    textOverflow: "ellipsis",
    transitionDuration: "100ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "ease-out",
    whiteSpace: "nowrap",
  },
  surface: {
    alignItems: "center",
    background: chatCta.surface,
    borderColor: chatCta.borderColor,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: 0,
    boxShadow: chatCta.surfaceShadow,
    display: "inline-flex",
    fontSize: "0.875rem",
    height: chatCta.height,
    left: "50%",
    overflow: "hidden",
    paddingInline: chatCta.paddingInline,
    pointerEvents: "none",
    position: "absolute",
    transform: "translateX(-50%)",
    transformOrigin: "bottom",
    transitionDuration: "150ms",
    transitionProperty:
      "width, height, padding, background-color, border-color, box-shadow",
    transitionTimingFunction: "cubic-bezier(0.22, 1, 0.36, 1)",
    width: chatCta.width,
  },
  trigger: {
    [chatCta.borderColor]: {
      default: "transparent",
      ":focus-visible": `color-mix(in oklab, ${colors.border} 70%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.border} 70%, transparent)`,
    },
    [chatCta.height]: {
      default: "0.5rem",
      ":focus-visible": "2.5rem",
      ":hover": "2.5rem",
      ":is(.dark *)": "0.75rem",
      ":is(.dark *):focus-visible": "2.5rem",
      ":is(.dark *):hover": "2.5rem",
    },
    [chatCta.labelColor]: {
      default: "rgb(255 255 255 / 0.55)",
      ":focus-within": colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
    [chatCta.labelOpacity]: {
      default: "0",
      ":focus-within": "1",
      ":hover": "1",
    },
    [chatCta.paddingInline]: {
      default: "0px",
      ":focus-visible": "1rem",
      ":hover": "1rem",
    },
    [chatCta.surface]: {
      default: "linear-gradient(180deg, #faf8f6 0%, #e3e1df 100%)",
      ":focus-visible": "#f4f4f5",
      ":hover": "#f4f4f5",
      ":is(.dark *)": "linear-gradient(180deg, #211d1d 0%, #574f3b 100%)",
      ":is(.dark *):focus-visible": "#202020",
      ":is(.dark *):hover": "#202020",
    },
    [chatCta.surfaceShadow]: {
      default:
        "0 0 0 1px rgba(0, 0, 0, 0.1), 0 4px 12px rgba(0, 0, 0, 0.16), 0 4px 16px rgba(0, 0, 0, 0.1), inset 0 -1px 0 rgba(0, 0, 0, 0.25), inset 0 1px 0 rgba(255, 255, 255, 0.4)",
      ":focus-visible": `0 0 0 2px ${colors.background}, 0 0 0 4px ${colors.ring}, ${expandedLightShadow}`,
      ":hover": expandedLightShadow,
      ":is(.dark *)":
        "0 4px 12px rgba(33, 29, 29, 0.1), inset 0 -1px 0 rgba(0, 0, 0, 0.25), inset 0 1px 0 rgba(255, 255, 255, 0.4)",
      ":is(.dark *):focus-visible": `0 0 0 2px ${colors.background}, 0 0 0 4px ${colors.ring}, ${expandedDarkShadow}`,
      ":is(.dark *):hover": expandedDarkShadow,
    },
    [chatCta.width]: {
      default: "180px",
      ":focus-visible": "min(640px, calc(100cqw - 2rem))",
      ":hover": "min(640px, calc(100cqw - 2rem))",
    },
    cursor: "text",
    height: "2.5rem",
    maxWidth: "100%",
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    position: "relative",
    width: "180px",
  },
});

export { styles as chatCtaStyles };
