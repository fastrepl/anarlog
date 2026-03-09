import { Icon } from "@iconify-icon/react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as sfxCommands } from "@hypr/plugin-sfx";

import { OnboardingButton } from "./shared";

import { commands } from "~/types/tauri.gen";

const SOCIALS = [
  {
    label: "Discord",
    icon: "simple-icons:discord",
    size: 14,
    url: "https://discord.gg/CX8gTH2tj9",
  },
  {
    label: "GitHub",
    icon: "simple-icons:github",
    size: 14,
    url: "https://github.com/fastrepl/char",
  },
  {
    label: "X",
    icon: "simple-icons:x",
    size: 10,
    url: "https://x.com/getcharnotes",
  },
] as const;

export function FinalSection({ onContinue }: { onContinue: () => void }) {
  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center gap-1.5 text-sm text-neutral-500">
        <span>Join our community and stay updated:</span>
        {SOCIALS.map(({ label, icon, size, url }) => {
          return (
            <button
              key={label}
              onClick={() => void openerCommands.openUrl(url, null)}
              className="inline-flex size-6 items-center justify-center rounded-md text-neutral-400 transition-colors duration-150 hover:text-neutral-700"
              aria-label={label}
            >
              <Icon icon={icon} width={size} height={size} />
            </button>
          );
        })}
      </div>

      <OnboardingButton onClick={() => void finishOnboarding(onContinue)}>
        Get Started
      </OnboardingButton>
    </div>
  );
}

export async function finishOnboarding(onContinue?: () => void) {
  await sfxCommands.stop("BGM").catch(console.error);
  await new Promise((resolve) => setTimeout(resolve, 100));
  await commands.setOnboardingNeeded(false).catch(console.error);
  await new Promise((resolve) => setTimeout(resolve, 100));
  await analyticsCommands.event({ event: "onboarding_completed" });
  onContinue?.();
}
