import { Icon } from "@iconify-icon/react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as sfxCommands } from "@hypr/plugin-sfx";

import { OnboardingButton } from "./shared";

import { flushAutomaticRelaunch } from "~/store/tinybase/store/save";
import { commands } from "~/types/tauri.gen";

const SOCIALS = [
  {
    label: "Discord",
    icon: "simple-icons:discord",
    size: 23,
    url: "https://discord.gg/CX8gTH2tj9",
  },
  {
    label: "GitHub",
    icon: "simple-icons:github",
    size: 23,
    url: "https://github.com/fastrepl/char",
  },
  {
    label: "X",
    icon: "simple-icons:x",
    size: 23,
    url: "https://x.com/getcharnotes",
  },
] as const;

export function FinalSection({ onContinue }: { onContinue: () => void }) {
  return (
    <div className="flex flex-col gap-6">
      <div className="items-left flex flex-col gap-4 text-sm text-neutral-500">
        <span>Join our community and stay updated:</span>
        <div className="flex items-center gap-4">
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
      </div>

      <OnboardingButton
        className="px-10 py-3.5 text-base"
        onClick={() => void finishOnboarding(onContinue)}
      >
        Open Char
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
  if (await flushAutomaticRelaunch()) {
    return;
  }
  onContinue?.();
}
