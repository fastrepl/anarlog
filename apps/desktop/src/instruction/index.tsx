import { Icon } from "@iconify-icon/react";
import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowSquareOut, CaretLeft, GithubLogo } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { OutlookIcon } from "@anlg/ui/components/icons/outlook";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";

import { useAuth } from "~/auth";

export type InstructionType = "sign-in" | "billing" | "integration";

function useInstructionCleanup(onCleanup?: () => void) {
  const cleanupRef = useRef(onCleanup);

  useEffect(() => {
    cleanupRef.current = onCleanup;
  }, [onCleanup]);

  useEffect(() => {
    return () => {
      cleanupRef.current?.();
    };
  }, []);
}

function InstructionShell({
  title,
  description,
  icon,
  onBack,
  action,
  children,
}: {
  title: ReactNode;
  description: ReactNode;
  icon?: ReactNode;
  onBack: () => void;
  action?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div {...stylex.props(styles.shell)}>
      <div {...stylex.props(styles.topGlow)} />

      <div data-tauri-drag-region {...stylex.props(styles.header)}>
        <button
          type="button"
          onClick={onBack}
          {...stylex.props(styles.backButton)}
        >
          <CaretLeft {...stylex.props(styles.backIcon)} />
          <span {...stylex.props(styles.backLabel)}>
            <Trans>Back</Trans>
          </span>
        </button>
      </div>

      <div data-tauri-drag-region {...stylex.props(styles.main)}>
        <div {...stylex.props(styles.content)}>
          {icon ?? (
            <img
              src="/assets/anarlog-icon.png"
              alt=""
              {...stylex.props(styles.defaultIcon)}
            />
          )}

          <div {...stylex.props(styles.copy)}>
            <h2 {...stylex.props(styles.title)}>{title}</h2>
            <p {...stylex.props(styles.description)}>{description}</p>
          </div>

          {action ? (
            <div {...stylex.props(styles.fullWidth)}>{action}</div>
          ) : null}
          {children ? (
            <div {...stylex.props(styles.children)}>{children}</div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function ExternalInstruction({
  title,
  description,
  icon,
  actionLabel,
  onBack,
  url,
}: {
  title: string;
  description: string;
  icon?: ReactNode;
  actionLabel: string;
  onBack: () => void;
  url?: string;
}) {
  return (
    <InstructionShell
      title={title}
      description={description}
      icon={icon}
      onBack={onBack}
      action={
        url ? (
          <Button
            variant="outline"
            sx={styles.externalAction}
            onClick={() => void openerCommands.openUrl(url, null)}
          >
            {actionLabel}
            <ArrowSquareOut {...stylex.props(styles.smallIcon)} />
          </Button>
        ) : undefined
      }
    />
  );
}

export function InstructionScreen({
  type,
  onBack,
  url,
  integrationId,
  onCleanup,
}: {
  type: InstructionType;
  onBack: () => void;
  url?: string;
  integrationId?: string;
  onCleanup?: () => void;
}) {
  const { t } = useLingui();
  useInstructionCleanup(onCleanup);

  if (type === "sign-in") {
    return <SignInInstruction onBack={onBack} />;
  }

  if (type === "billing") {
    return (
      <ExternalInstruction
        title={t`Complete your purchase`}
        description={t`Finish checkout in your browser, then return to Anarlog.`}
        actionLabel={t`Reopen checkout page`}
        onBack={onBack}
        url={url}
      />
    );
  }

  const integration = getIntegrationInstruction(integrationId);

  return (
    <ExternalInstruction
      title={
        integration
          ? t`Connect ${integration.displayName}`
          : t`Connect your integration`
      }
      description={t`Authorize access in your browser, then return to Anarlog.`}
      icon={integration?.icon}
      actionLabel={t`Reopen in browser`}
      onBack={onBack}
      url={url}
    />
  );
}

function getIntegrationInstruction(integrationId?: string):
  | {
      displayName: string;
      icon: ReactNode;
    }
  | undefined {
  switch (integrationId) {
    case "google-calendar":
      return {
        displayName: "Google Calendar",
        icon: <Icon icon="logos:google-calendar" width={56} height={56} />,
      };
    case "outlook":
      return {
        displayName: "Outlook",
        icon: <OutlookIcon size={56} />,
      };
    case "github":
      return {
        displayName: "GitHub",
        icon: (
          <GithubLogo {...stylex.props(styles.githubIcon)} weight="light" />
        ),
      };
    case "slack":
      return {
        displayName: "Slack",
        icon: <Icon icon="logos:slack-icon" width={56} height={56} />,
      };
    default:
      return undefined;
  }
}

function SignInInstruction({ onBack }: { onBack: () => void }) {
  const { t } = useLingui();
  const auth = useAuth();
  const [callbackUrl, setCallbackUrl] = useState("");
  const [showCallbackInput, setShowCallbackInput] = useState(false);

  useEffect(() => {
    if (!auth?.session) {
      return;
    }

    onBack();
  }, [auth?.session, onBack]);

  return (
    <InstructionShell
      title={t`Sign in to your account`}
      description={t`Complete sign-in in your browser, then return to Anarlog.`}
      onBack={onBack}
    >
      {showCallbackInput ? (
        <>
          <div {...stylex.props(styles.callbackForm)}>
            <Input
              type="text"
              sx={styles.callbackInput}
              placeholder="anarlog://auth/callback?access_token=..."
              value={callbackUrl}
              onChange={(e) => setCallbackUrl(e.target.value)}
            />
            <Button
              sx={styles.tallControl}
              onClick={() => void auth.handleAuthCallback(callbackUrl)}
              disabled={!callbackUrl}
            >
              <Trans>Submit callback URL</Trans>
            </Button>
          </div>
          <p {...stylex.props(styles.callbackHint)}>
            <Trans>
              Paste the browser URL here if the browser button did not reopen
              Anarlog.
            </Trans>
          </p>
        </>
      ) : (
        <button
          type="button"
          onClick={() => setShowCallbackInput(true)}
          {...stylex.props(styles.handoffButton)}
        >
          <Trans>
            Browser handoff not working? Paste the callback link instead
          </Trans>
        </button>
      )}
    </InstructionShell>
  );
}

const styles = stylex.create({
  backButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.muted} 70%, transparent)`,
    },
    borderRadius: radii.full,
    color: colors.mutedForeground,
    display: "flex",
    gap: "0.375rem",
    height: "2.25rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  backIcon: {
    height: "1rem",
    width: "1rem",
  },
  backLabel: {
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
  },
  callbackForm: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    width: "100%",
  },
  callbackHint: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
  },
  callbackInput: {
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    height: "2.5rem",
    lineHeight: "1rem",
  },
  children: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    width: "100%",
  },
  content: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
    maxWidth: "24rem",
    paddingBottom: "2.5rem",
    paddingInline: "2.5rem",
    textAlign: "center",
    width: "100%",
  },
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    maxWidth: "100%",
  },
  defaultIcon: {
    height: "3.5rem",
    objectFit: "contain",
    objectPosition: "center",
    width: "3.5rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.5rem",
  },
  externalAction: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.background,
    },
    borderColor: colors.border,
    color: colors.mutedForeground,
    height: "2.5rem",
    width: "100%",
  },
  fullWidth: {
    width: "100%",
  },
  githubIcon: {
    color: colors.foreground,
    height: "3.5rem",
    width: "3.5rem",
  },
  handoffButton: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
    textDecorationLine: "underline",
    textUnderlineOffset: "4px",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  header: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    paddingInline: "0.75rem",
    paddingTop: "3rem",
    position: "relative",
    zIndex: 10,
  },
  main: {
    alignItems: "center",
    display: "flex",
    flex: 1,
    justifyContent: "center",
    padding: "1.5rem",
    position: "relative",
    zIndex: 10,
  },
  shell: {
    backgroundImage: `linear-gradient(to bottom, ${colors.background}, ${colors.card}, ${colors.card})`,
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    position: "relative",
    userSelect: "none",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  tallControl: {
    height: "2.5rem",
  },
  title: {
    color: colors.foreground,
    fontFamily: fonts.sans,
    fontSize: {
      default: "1.375rem",
      [media.sm]: "1.75rem",
    },
    fontWeight: 600,
    lineHeight: 1.15,
    overflowWrap: "break-word",
  },
  topGlow: {
    backgroundImage: `linear-gradient(to bottom, color-mix(in srgb, ${colors.muted} 40%, transparent), transparent)`,
    height: "8rem",
    left: 0,
    pointerEvents: "none",
    position: "absolute",
    right: 0,
    top: 0,
  },
});
