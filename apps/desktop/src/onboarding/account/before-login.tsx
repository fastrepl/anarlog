import { Trans } from "@lingui/react/macro";

import { OnboardingButton } from "../shared";

import { useAuth } from "~/auth";

export function BeforeLogin({ onContinue: _ }: { onContinue: () => void }) {
  const auth = useAuth();

  return (
    <div className="flex flex-col items-start">
      <div className="flex flex-row items-center gap-4">
        <OnboardingButton
          onClick={() => {
            void auth?.signIn();
          }}
          className="px-6 py-2 text-sm"
        >
          <Trans>Get started for free</Trans>
        </OnboardingButton>

        <button
          type="button"
          onClick={() => {
            void auth?.signIn();
          }}
          className="border-border/60 bg-card/55 text-muted-foreground hover:bg-card/75 hover:text-foreground rounded-full border px-6 py-2 text-sm font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.55)] backdrop-blur-sm transition-colors"
        >
          <Trans>Login with existing account</Trans>
        </button>
      </div>
    </div>
  );
}
