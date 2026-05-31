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
          Get started for free
        </OnboardingButton>

        <button
          type="button"
          onClick={() => {
            void auth?.signIn();
          }}
          className="rounded-full border border-white/60 bg-white/55 px-6 py-2 text-sm font-medium text-neutral-600 shadow-[inset_0_1px_0_rgba(255,255,255,0.55)] backdrop-blur-sm transition-colors hover:bg-white/75 hover:text-neutral-800"
        >
          Login with existing account
        </button>
      </div>
    </div>
  );
}
