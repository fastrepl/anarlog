import { CircleNotch, XCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import { Streamdown } from "streamdown";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { markdownComponents } from "@anlg/ui/components/markdown";

import { extractMcpOutputText } from "~/chat/mcp/mcp-output-parser";

export function ToolCard({
  failed,
  children,
}: {
  failed?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      {...stylex.props([
        styles.card,
        failed ? styles.failedCard : styles.defaultCard,
      ])}
    >
      {children}
    </div>
  );
}

export function ToolCardHeader({
  icon,
  running,
  failed,
  done,
  label,
}: {
  icon: ReactNode;
  running: boolean;
  failed: boolean;
  done: boolean;
  label: string;
}) {
  return (
    <div
      {...stylex.props([
        styles.header,
        failed ? styles.failedHeader : styles.defaultHeader,
      ])}
    >
      {running ? (
        <CircleNotch {...stylex.props([styles.headerIcon, styles.spinner])} />
      ) : (
        <span
          {...stylex.props([
            styles.iconSlot,
            failed
              ? styles.failedIcon
              : done
                ? styles.doneIcon
                : styles.defaultIcon,
          ])}
        >
          {icon}
        </span>
      )}
      <span {...stylex.props(styles.label)}>{label}</span>
    </div>
  );
}

export function ToolCardBody({ children }: { children: ReactNode }) {
  return <div {...stylex.props(styles.body)}>{children}</div>;
}

export function ToolCardFooterError({ text }: { text: string }) {
  return (
    <div {...stylex.props(styles.errorFooter)}>
      <XCircle {...stylex.props(styles.errorFooterIcon)} />
      <p {...stylex.props(styles.errorFooterText)}>{text}</p>
    </div>
  );
}

function ToolCardFooterRaw({ text }: { text: string }) {
  return (
    <div {...stylex.props(styles.rawFooter)}>
      <p {...stylex.props(styles.rawFooterText)}>{text}</p>
    </div>
  );
}

export function useToolState(part: { state: string }) {
  const running =
    part.state === "input-streaming" || part.state === "input-available";
  const failed = part.state === "output-error";
  const done = part.state === "output-available";
  return { running, failed, done };
}

export function useMcpOutput<T>(
  done: boolean,
  output: unknown,
  parseFn: (output: unknown) => T | null,
): { parsed: T | null; rawText: string | null } {
  const parsed = done ? parseFn(output) : null;
  const rawText = done && !parsed ? extractMcpOutputText(output) : null;
  return { parsed, rawText };
}

export function ToolCardFooters({
  failed,
  errorText,
  rawText,
  children,
}: {
  failed: boolean;
  errorText?: unknown;
  rawText: string | null;
  children?: ReactNode;
}) {
  return (
    <>
      {children}
      {failed ? (
        <ToolCardFooterError text={String(errorText ?? "Unknown error")} />
      ) : null}
      {rawText ? <ToolCardFooterRaw text={rawText} /> : null}
    </>
  );
}

export function MarkdownPreview({ children }: { children: string }) {
  return (
    <div {...stylex.props(styles.markdownPreview)}>
      <div {...stylex.props(styles.markdownViewport)}>
        <Streamdown
          {...stylex.props(styles.markdown)}
          components={markdownComponents}
          controls={false}
          linkSafety={{ enabled: false }}
        >
          {children}
        </Streamdown>
      </div>
    </div>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  card: {
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    marginBlock: "0.625rem",
    overflow: "hidden",
  },
  failedCard: {
    borderColor: colors.alertBorder,
  },
  defaultCard: {
    borderColor: `color-mix(in oklab, ${colors.border} 80%, transparent)`,
  },
  header: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.8125rem",
    gap: "0.625rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.875rem",
  },
  failedHeader: {
    backgroundColor: colors.alert,
    color: "oklch(50.5% 0.213 27.518)",
  },
  defaultHeader: {
    backgroundColor: `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
    color: colors.mutedForeground,
  },
  headerIcon: {
    height: "1rem",
    width: "1rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  iconSlot: {
    flexShrink: 0,
    fontSize: "1rem",
    lineHeight: 1,
  },
  failedIcon: {
    color: "oklch(63.7% 0.237 25.331)",
  },
  doneIcon: {
    color: "oklch(69.6% 0.17 162.48)",
  },
  defaultIcon: {
    color: colors.mutedForeground,
  },
  label: {
    fontWeight: 500,
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "0.625rem",
    paddingBlock: "0.625rem",
    paddingInline: "0.875rem",
  },
  errorFooter: {
    alignItems: "center",
    backgroundColor: colors.alert,
    borderColor: colors.alertBorder,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    paddingBlock: "0.625rem",
    paddingInline: "0.875rem",
  },
  errorFooterIcon: {
    color: "oklch(63.7% 0.237 25.331)",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  errorFooterText: {
    color: "oklch(57.7% 0.245 27.325)",
    fontSize: "0.8125rem",
  },
  rawFooter: {
    backgroundColor: `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
    borderColor: `color-mix(in oklab, ${colors.border} 80%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBlock: "0.625rem",
    paddingInline: "0.875rem",
  },
  rawFooterText: {
    color: colors.mutedForeground,
    fontSize: "0.8125rem",
    whiteSpace: "pre-wrap",
  },
  markdownPreview: {
    backgroundColor: colors.card,
    borderColor: `color-mix(in oklab, ${colors.border} 80%, transparent)`,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
  },
  markdownViewport: {
    maxHeight: "16rem",
    overflowY: "auto",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
  },
  markdown: {
    color: colors.mutedForeground,
    fontSize: "0.8125rem",
    lineHeight: 1.625,
  },
});
