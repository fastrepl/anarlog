import { useLingui } from "@lingui/react";
import { Trans } from "@lingui/react/macro";
import {
  CircleNotch,
  DiscordLogo,
  GithubLogo,
  XLogo,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as sfxCommands } from "@anlg/plugin-sfx";

import { OnboardingButton, onboardingSharedStyles } from "./shared";
import {
  getOrCreateWelcomeSession,
  setPendingWelcomeSession,
} from "./welcome-note";

import { createSession } from "~/session/queries";
import { flushAutomaticRelaunch } from "~/shared/relaunch";
import { commands } from "~/types/tauri.gen";

const SOCIALS = [
  {
    label: "Discord",
    icon: DiscordLogo,
    url: "https://anarlog.so/discord",
  },
  {
    label: "GitHub",
    icon: GithubLogo,
    url: "https://github.com/fastrepl/anarlog",
  },
  {
    label: "X",
    icon: XLogo,
    size: 14,
    url: "https://x.com/anarlogapp",
  },
] as const;

const SOCIAL_ICON_SIZE = 18;

export function FinalDescription() {
  return (
    <div {...stylex.props(styles.description)}>
      <span>
        <Trans>Join our community and stay updated:</Trans>
      </span>
      <div {...stylex.props(styles.socials)}>
        {SOCIALS.map((social) => {
          const iconSize = "size" in social ? social.size : SOCIAL_ICON_SIZE;
          const SocialIcon = social.icon;

          return (
            <button
              key={social.label}
              onClick={() => void openerCommands.openUrl(social.url, null)}
              {...stylex.props(styles.socialButton)}
              aria-label={social.label}
            >
              <SocialIcon size={iconSize} />
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function FinalSection({
  onContinue,
}: {
  onContinue: (sessionId: string) => void;
}) {
  const { i18n } = useLingui();
  const translate = i18n._.bind(i18n);
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");
  const finishPromiseRef = useRef<Promise<void> | null>(null);
  const welcomeSessionRef = useRef<string | null>(null);

  const handleContinue = async () => {
    if (finishPromiseRef.current) return;

    setStatus("loading");
    const finishPromise = finishOnboarding(onContinue, welcomeSessionRef);
    finishPromiseRef.current = finishPromise;
    try {
      await finishPromise;
    } catch (error) {
      console.error("Failed to finish onboarding", error);
      setStatus("error");
    } finally {
      finishPromiseRef.current = null;
    }
  };

  return (
    <div {...stylex.props(styles.root)}>
      <OnboardingButton
        sx={[onboardingSharedStyles.compactButton, styles.continueButton]}
        disabled={status === "loading"}
        onClick={() => void handleContinue()}
      >
        {status === "loading" ? (
          <span {...stylex.props(styles.loadingLabel)}>
            <CircleNotch
              {...stylex.props([
                styles.loadingIcon,
                onboardingSharedStyles.spin,
              ])}
            />
            <Trans>Open Anarlog</Trans>
          </span>
        ) : (
          <Trans>Open Anarlog</Trans>
        )}
      </OnboardingButton>
      {status === "error" && (
        <p {...stylex.props(styles.error)} role="alert">
          {translate({
            id: "onboarding.finish-error",
            message: "Couldn't open Anarlog. Please try again.",
          })}
        </p>
      )}
    </div>
  );
}

export async function finishOnboarding(
  onContinue?: (sessionId: string) => void,
  welcomeSessionRef?: { current: string | null },
) {
  await sfxCommands.stop("BGM").catch(console.error);
  const welcomeSessionId =
    welcomeSessionRef?.current ??
    (await getOrCreateWelcomeSession().catch((error) => {
      console.error("Failed to create welcome note", error);
      return createSession();
    }));
  if (welcomeSessionRef) {
    welcomeSessionRef.current = welcomeSessionId;
  }
  await new Promise((resolve) => setTimeout(resolve, 100));
  const result = await commands.setOnboardingNeeded(false);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  await new Promise((resolve) => setTimeout(resolve, 100));
  void analyticsCommands
    .event({ event: "onboarding_completed" })
    .catch(console.error);
  setPendingWelcomeSession(welcomeSessionId);
  if (await flushAutomaticRelaunch()) {
    return;
  }
  setPendingWelcomeSession(null);
  onContinue?.(welcomeSessionId);
}

const styles = stylex.create({
  continueButton: {
    cursor: {
      default: "pointer",
      ":disabled": "wait",
    },
    fontSize: "0.875rem",
    opacity: {
      default: 1,
      ":disabled": 0.7,
    },
  },
  description: {
    alignItems: "center",
    columnGap: "0.75rem",
    display: "flex",
    flexWrap: "wrap",
    rowGap: "0.5rem",
  },
  error: {
    color: "rgb(239 68 68)",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  loadingIcon: {
    height: "1rem",
    width: "1rem",
  },
  loadingLabel: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  root: {
    alignItems: "flex-start",
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  socialButton: {
    alignItems: "center",
    borderRadius: radii.md,
    color: colors.mutedForeground,
    display: "inline-flex",
    height: "1.25rem",
    justifyContent: "center",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.25rem",
  },
  socials: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
});
