import { Trans, useLingui } from "@lingui/react/macro";
import {
  CaretLeft,
  CaretRight,
  Check,
  CheckCircle,
  CircleNotch,
  XCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useEffect, useRef } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";

const SCROLL_DELAY_MS = 350;

export type SectionStatus = "completed" | "active" | "upcoming";

export function OnboardingSection({
  title,
  completedTitle,
  description,
  status,
  onBack,
  onNext,
  onSkip,
  skippable = true,
  children,
}: {
  title: ReactNode;
  completedTitle?: ReactNode;
  description?: ReactNode;
  status: SectionStatus | null;
  onBack?: () => void;
  onNext?: () => void;
  onSkip?: () => void;
  skippable?: boolean;
  children: ReactNode;
}) {
  const { t } = useLingui();
  const sectionRef = useRef<HTMLElement>(null);

  const isActive = status === "active";
  const isCompleted = status === "completed";

  useEffect(() => {
    if (!isActive) return;
    const timeout = setTimeout(() => {
      sectionRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "start",
      });
    }, SCROLL_DELAY_MS);
    return () => clearTimeout(timeout);
  }, [isActive]);

  if (!status || status === "upcoming") return null;

  return (
    <section ref={sectionRef}>
      <div
        {...stylex.props([
          styles.sectionHeader,
          isActive && styles.activeHeader,
        ])}
      >
        {isCompleted && (
          <Check {...stylex.props(styles.completedIcon)} weight="bold" />
        )}
        <div {...stylex.props(styles.sectionCopy)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2
              {...stylex.props([
                styles.sectionTitle,
                isCompleted ? styles.completedTitle : styles.activeTitle,
              ])}
            >
              {isCompleted ? (completedTitle ?? title) : title}
            </h2>
            {isActive && (
              <div {...stylex.props(styles.navigation)}>
                {import.meta.env.DEV && onBack && (
                  <button
                    onClick={onBack}
                    aria-label={t`Go to previous section`}
                    {...stylex.props(styles.navigationButton)}
                  >
                    <CaretLeft {...stylex.props(styles.navigationIcon)} />
                  </button>
                )}
                {onNext &&
                  (skippable ? (
                    <button
                      onClick={() => {
                        if (onSkip) {
                          onSkip();
                        } else {
                          onNext?.();
                        }
                      }}
                      {...stylex.props(styles.skipButton)}
                    >
                      <Trans>Skip</Trans>
                      <CaretRight {...stylex.props(styles.navigationIcon)} />
                    </button>
                  ) : import.meta.env.DEV ? (
                    <button
                      onClick={onNext}
                      aria-label={t`Go to next section`}
                      {...stylex.props(styles.navigationButton)}
                    >
                      <CaretRight {...stylex.props(styles.navigationIcon)} />
                    </button>
                  ) : null)}
              </div>
            )}
          </div>
          {isActive && description && (
            <div {...stylex.props(styles.sectionDescription)}>
              {description}
            </div>
          )}
        </div>
      </div>

      <AnimatePresence initial={false}>
        {isActive && (
          <motion.div
            key="content"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3, ease: "easeInOut" }}
            {...stylex.props(styles.sectionContent)}
          >
            {children}
          </motion.div>
        )}
      </AnimatePresence>
    </section>
  );
}

export function OnboardingButton({
  variant = "primary",
  sx,
  ...props
}: {
  variant?: "primary" | "secondary" | "ghost";
  sx?: stylex.StyleXStyles;
} & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "className">) {
  return (
    <button
      {...props}
      {...stylex.props([
        styles.onboardingButton,
        onboardingButtonVariants[variant],
        sx,
      ])}
    />
  );
}

export function StepRow({
  status,
  label,
}: {
  status: "done" | "active" | "failed";
  label: ReactNode;
}) {
  return (
    <div {...stylex.props(styles.stepRow)}>
      {status === "done" && <CheckCircle {...stylex.props(styles.doneIcon)} />}
      {status === "active" && (
        <CircleNotch {...stylex.props([styles.stepIcon, styles.spin])} />
      )}
      {status === "failed" && <XCircle {...stylex.props(styles.failedIcon)} />}
      <span
        {...stylex.props(
          status === "failed" ? styles.failedLabel : styles.stepLabel,
        )}
      >
        {label}
      </span>
    </div>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

export const onboardingSharedStyles = stylex.create({
  compactButton: {
    paddingBlock: "0.5rem",
    paddingInline: "1.5rem",
  },
  spin: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
});

const styles = stylex.create({
  activeHeader: {
    marginBottom: "0.75rem",
    paddingTop: "1rem",
  },
  activeTitle: {
    color: colors.foreground,
    fontFamily: fonts.sans,
    fontSize: "1.25rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
  },
  completedIcon: {
    color: "rgb(22 163 74)",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  completedTitle: {
    color: `color-mix(in srgb, ${colors.mutedForeground} 70%, transparent)`,
    fontSize: "0.75rem",
    fontWeight: 400,
    lineHeight: "1rem",
  },
  doneIcon: {
    color: "rgb(5 150 105)",
    height: "1rem",
    width: "1rem",
  },
  failedIcon: {
    color: "rgb(248 113 113)",
    height: "1rem",
    width: "1rem",
  },
  failedLabel: {
    color: "rgb(239 68 68)",
  },
  ghostButton: {
    color: colors.mutedForeground,
  },
  navigation: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  navigationButton: {
    borderRadius: "0.25rem",
    color: colors.mutedForeground,
    padding: "0.125rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  navigationIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  onboardingButton: {
    borderRadius: radii.full,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    paddingBlock: "0.625rem",
    paddingInline: "1.5rem",
    transitionDuration: "200ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "fit-content",
  },
  primaryButton: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderColor: colors.primary,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow:
      "0 2px 6px rgb(87 83 78 / 0.22), 0 10px 18px -10px rgb(87 83 78 / 0.65)",
    color: colors.primaryForeground,
  },
  secondaryButton: {
    backdropFilter: "blur(4px)",
    backgroundColor: {
      default: `color-mix(in srgb, ${colors.card} 55%, transparent)`,
      ":hover": `color-mix(in srgb, ${colors.card} 75%, transparent)`,
    },
    borderColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "inset 0 1px 0 rgb(255 255 255 / 0.55)",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
  },
  sectionContent: {
    marginBottom: "-1.25rem",
    marginInline: "-1.25rem",
    overflow: "hidden",
    paddingBottom: "1.25rem",
    paddingInline: "1.25rem",
    paddingTop: "0.75rem",
  },
  sectionCopy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    minWidth: 0,
  },
  sectionDescription: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  sectionHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    transitionDuration: "300ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  sectionTitle: {
    transitionDuration: "300ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  skipButton: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.25rem",
    lineHeight: "1.25rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  spin: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  stepIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  stepLabel: {
    color: colors.mutedForeground,
  },
  stepRow: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  titleRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
});

const onboardingButtonVariants = {
  ghost: styles.ghostButton,
  primary: styles.primaryButton,
  secondary: styles.secondaryButton,
};
