import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { OnboardingButton, OnboardingCharIcon } from "../shared";

import { useAuth } from "~/auth";

export function BeforeLogin({ onContinue }: { onContinue: () => void }) {
  const auth = useAuth();

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col items-start gap-2">
        <div className="flex items-center gap-3">
          <OnboardingButton
            onClick={() => {
              void auth?.signIn();
            }}
            className="flex items-center gap-2"
          >
            <OnboardingCharIcon inverted />
            Sign in
          </OnboardingButton>
          <button
            type="button"
            onClick={() => {
              void analyticsCommands.event({
                event: "onboarding_login_skipped",
              });
              onContinue();
            }}
            className="text-sm text-neutral-500/70 transition-colors hover:text-neutral-700"
          >
            Skip for now
          </button>
        </div>
      </div>
    </div>
  );
}
