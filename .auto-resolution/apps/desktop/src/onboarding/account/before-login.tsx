import { Trans } from "@lingui/react/macro";
import { Loader2Icon } from "lucide-react";
import { useState } from "react";

import { OnboardingButton } from "../shared";

import { useAuth } from "~/auth";

export function BeforeLogin({ onContinue: _ }: { onContinue: () => void }) {
  const auth = useAuth();
  const [isOpening, setIsOpening] = useState(false);

  const handleSignIn = () => {
    if (isOpening) return;
    setIsOpening(true);
    void auth.signIn().finally(() => setIsOpening(false));
  };

  return (
    <div className="flex flex-col items-start">
      <div className="flex flex-row items-center gap-4">
        <OnboardingButton
          onClick={handleSignIn}
          disabled={isOpening}
          className="flex items-center gap-2 px-6 py-2 text-sm disabled:opacity-70"
        >
          {isOpening ? (
            <Loader2Icon className="size-3.5 animate-spin" aria-hidden="true" />
          ) : null}
          <Trans>Get started</Trans>
        </OnboardingButton>

        <button
          type="button"
          onClick={handleSignIn}
          disabled={isOpening}
          className="border-border/60 bg-card/55 text-muted-foreground hover:bg-card/75 hover:text-foreground rounded-full border px-6 py-2 text-sm font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.55)] backdrop-blur-sm transition-colors disabled:opacity-50"
        >
          <Trans>Login</Trans>
        </button>
      </div>
    </div>
  );
}
