import { CaretRight, CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";

export function MessageContainer({
  align = "start",
  children,
}: {
  align?: "start" | "end";
  children: ReactNode;
}) {
  return (
    <div
      {...stylex.props([
        styles.messageContainer,
        align === "end" ? styles.alignEnd : styles.alignStart,
      ])}
    >
      {children}
    </div>
  );
}

export function MessageBubble({
  variant = "assistant",
  withActionButton,
  children,
}: {
  variant?: "user" | "assistant" | "error" | "loading";
  withActionButton?: boolean;
  children: ReactNode;
}) {
  const { isDarkAppearance } = useChatAppearance();

  return (
    <div
      data-chat-action-parent={withActionButton || undefined}
      data-chat-message-appearance={isDarkAppearance ? "dark" : "light"}
      data-chat-message-variant={variant}
      {...stylex.props([
        styles.messageBubble,
        messageBubbleVariantStyles[variant],
        isDarkAppearance &&
          variant === "assistant" &&
          styles.darkAssistantBubble,
        isDarkAppearance && variant === "loading" && styles.darkLoadingBubble,
        withActionButton && styles.actionParent,
      ])}
    >
      {children}
    </div>
  );
}

export function ActionButton({
  onClick,
  variant = "default",
  icon: Icon,
  label,
}: {
  onClick: () => void;
  variant?: "default" | "error";
  icon: React.ComponentType<{
    className?: string;
    style?: React.CSSProperties;
  }>;
  label: string;
}) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      {...stylex.props([
        styles.actionButton,
        actionButtonVariantStyles[variant],
      ])}
    >
      <Icon {...stylex.props(styles.smallIcon)} />
    </button>
  );
}

export function Disclosure({
  icon,
  title,
  children,
  disabled,
}: {
  icon: ReactNode;
  title: ReactNode;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <details data-chat-disclosure {...stylex.props(styles.disclosure)}>
      <summary
        onClick={(event) => {
          if (disabled) {
            event.preventDefault();
          }
        }}
        {...stylex.props([styles.summary, disabled && styles.disabledSummary])}
      >
        {disabled ? (
          <CircleNotch {...stylex.props([styles.smallIcon, styles.spinner])} />
        ) : null}
        {!disabled && icon && (
          <span {...stylex.props(styles.iconSlot)}>{icon}</span>
        )}
        <span {...stylex.props(styles.summaryTitle)}>{title}</span>
        <CaretRight {...stylex.props(styles.disclosureCaret)} />
      </summary>
      <div {...stylex.props(styles.disclosureContent)}>{children}</div>
    </details>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  messageContainer: {
    display: "flex",
    paddingBlock: "0.5rem",
  },
  alignStart: {
    justifyContent: "flex-start",
  },
  alignEnd: {
    justifyContent: "flex-end",
  },
  messageBubble: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    userSelect: {
      default: "text",
      ":is(*) *": "text",
    },
  },
  userBubble: {
    backgroundColor: "oklch(93.2% 0.032 255.585)",
    borderRadius: "1rem",
    color: "oklch(26.9% 0 0)",
    maxWidth: "100%",
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    textWrap: "wrap",
    width: "fit-content",
  },
  assistantBubble: {
    color: colors.foreground,
  },
  loadingBubble: {
    color: colors.foreground,
  },
  darkAssistantBubble: {
    backgroundColor: colors.accent,
    borderRadius: "1rem",
    color: colors.accentForeground,
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
  },
  darkLoadingBubble: {
    backgroundColor: colors.accent,
    borderRadius: "1rem",
    color: colors.accentForeground,
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    width: "fit-content",
  },
  errorBubble: {
    backgroundColor: colors.alert,
    borderColor: colors.alertBorder,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.alertForeground,
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
  },
  actionParent: {
    position: "relative",
  },
  actionButton: {
    borderRadius: radii.full,
    opacity: {
      default: 0,
      ":is([data-chat-action-parent]:hover *)": 1,
    },
    padding: "0.25rem",
    position: "absolute",
    right: "-0.25rem",
    top: "-0.25rem",
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  defaultActionButton: {
    backgroundColor: {
      default: colors.accent,
      ":hover": colors.accent,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  errorActionButton: {
    backgroundColor: {
      default: "oklch(93.6% 0.032 17.717)",
      ":hover": "oklch(88.5% 0.062 18.334)",
    },
    color: {
      default: "oklch(57.7% 0.245 27.325)",
      ":hover": "oklch(44.4% 0.177 26.899)",
    },
  },
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  disclosure: {
    borderColor: {
      default: colors.border,
      ":hover": colors.border,
    },
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    cursor: "pointer",
    marginBlock: "0.5rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  summary: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
    lineHeight: "1rem",
    listStyle: "none",
    userSelect: "none",
    width: "100%",
  },
  disabledSummary: {
    cursor: "default",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  iconSlot: {
    flexShrink: 0,
  },
  summaryTitle: {
    flex: "1",
    fontWeight: {
      default: 400,
      ":is([data-chat-disclosure][open] *)": 500,
    },
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  disclosureCaret: {
    flexShrink: 0,
    height: "0.75rem",
    transform: {
      default: "rotate(0deg)",
      ":is([data-chat-disclosure][open] *)": "rotate(90deg)",
    },
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "0.75rem",
  },
  disclosureContent: {
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    marginTop: "0.25rem",
    paddingInline: "0.25rem",
    paddingTop: "0.5rem",
  },
});

const messageBubbleVariantStyles = {
  assistant: styles.assistantBubble,
  error: styles.errorBubble,
  loading: styles.loadingBubble,
  user: styles.userBubble,
};

const actionButtonVariantStyles = {
  default: styles.defaultActionButton,
  error: styles.errorActionButton,
};
