import { Trans, useLingui } from "@lingui/react/macro";
import { Lock } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

export function LockScreen({
  title,
  description,
  action,
  authenticating,
  onUnlock,
  sx,
}: {
  title: string;
  description?: string;
  action?: string;
  authenticating: boolean;
  onUnlock: () => void;
  sx?: stylex.StyleXStyles;
}) {
  return (
    <div
      data-lock-screen
      data-tauri-drag-region
      {...stylex.props([styles.screen, styles.screenBackground, sx])}
    >
      <div {...stylex.props(styles.content)}>
        <div {...stylex.props(styles.iconContainer)}>
          <Lock {...stylex.props(styles.icon)} weight="fill" />
        </div>
        <h1 {...stylex.props(styles.title)}>{title}</h1>
        {description ? (
          <p {...stylex.props(styles.description)}>{description}</p>
        ) : null}
        <Button
          sx={styles.action}
          disabled={authenticating}
          onClick={onUnlock}
          data-tauri-drag-region="false"
        >
          {authenticating ? (
            <Trans>Authenticating…</Trans>
          ) : action ? (
            action
          ) : (
            <Trans>Unlock</Trans>
          )}
        </Button>
      </div>
    </div>
  );
}

export function useDeviceAuthHint() {
  const { t } = useLingui();
  switch (platform()) {
    case "macos":
      return t`Use Touch ID or enter your password to view.`;
    case "windows":
      return t`Use Windows Hello face, PIN, or password to view.`;
    default:
      return t`Authenticate to continue.`;
  }
}

export function NoteLockScreen({
  sessionTitle,
  authenticating,
  onUnlock,
}: {
  sessionTitle?: string;
  authenticating: boolean;
  onUnlock: () => void;
}) {
  const { t } = useLingui();
  const hint = useDeviceAuthHint();
  return (
    <LockScreen
      title={sessionTitle || t`Note is Locked`}
      description={hint}
      action={t`View Note`}
      authenticating={authenticating}
      onUnlock={onUnlock}
      sx={styles.transparent}
    />
  );
}

const styles = stylex.create({
  action: {
    marginTop: "1.5rem",
  },
  content: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    maxWidth: "24rem",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginTop: "0.5rem",
  },
  icon: {
    color: colors.foreground,
    height: "1.5rem",
    width: "1.5rem",
  },
  iconContainer: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: "9999px",
    display: "flex",
    height: "3.5rem",
    justifyContent: "center",
    width: "3.5rem",
  },
  screen: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
    justifyContent: "center",
    minHeight: 0,
    width: "100%",
  },
  screenBackground: {
    backgroundColor: colors.background,
  },
  title: {
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
    marginTop: "1.25rem",
  },
  transparent: {
    backgroundColor: "transparent",
  },
});
