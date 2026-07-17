import { Icon } from "@iconify-icon/react";
import { Trans } from "@lingui/react/macro";
import { Loader2Icon } from "lucide-react";
import { useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as sfxCommands } from "@hypr/plugin-sfx";

import { OnboardingButton } from "./shared";
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
    icon: "simple-icons:discord",
    url: "https://anarlog.so/discord",
  },
  {
    label: "GitHub",
    icon: "simple-icons:github",
    url: "https://github.com/fastrepl/char",
  },
  {
    label: "X",
    icon: "simple-icons:x",
    size: 14,
    url: "https://x.com/getcharnotes",
  },
] as const;

const SOCIAL_ICON_SIZE = 18;

export function FinalDescription() {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
      <span>
        <Trans>Join our community and stay updated:</Trans>
      </span>
      <div className="flex items-center gap-2">
        {SOCIALS.map((social) => {
          const iconSize = "size" in social ? social.size : SOCIAL_ICON_SIZE;

          return (
            <button
              key={social.label}
              onClick={() => void openerCommands.openUrl(social.url, null)}
              className="text-muted-foreground hover:text-muted-foreground inline-flex size-5 items-center justify-center rounded-md transition-colors duration-150"
              aria-label={social.label}
            >
              <Icon icon={social.icon} width={iconSize} height={iconSize} />
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
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");

  const handleContinue = async () => {
    if (status === "loading") return;

    setStatus("loading");
    try {
      await finishOnboarding(onContinue);
    } catch (error) {
      console.error("Failed to finish onboarding", error);
      setStatus("error");
    }
  };

  return (
    <div className="flex flex-col items-start gap-2">
      <OnboardingButton
        className="px-6 py-2 text-sm disabled:cursor-wait disabled:opacity-70"
        disabled={status === "loading"}
        onClick={() => void handleContinue()}
      >
        {status === "loading" ? (
          <span className="flex items-center gap-2">
            <Loader2Icon className="size-4 animate-spin" />
            <Trans>Open Anarlog</Trans>
          </span>
        ) : (
          <Trans>Open Anarlog</Trans>
        )}
      </OnboardingButton>
      {status === "error" && (
        <p className="text-sm text-red-500" role="alert">
          <Trans>Retry</Trans>
        </p>
      )}
    </div>
  );
}

export async function finishOnboarding(
  onContinue?: (sessionId: string) => void,
) {
  await sfxCommands.stop("BGM").catch(console.error);
  const welcomeSessionId = await getOrCreateWelcomeSession().catch((error) => {
    console.error("Failed to create welcome note", error);
    return createSession();
  });
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
