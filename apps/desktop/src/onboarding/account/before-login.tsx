import { useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { OnboardingButton } from "../shared";

import { useAuth } from "~/auth";

export function BeforeLogin({ onContinue }: { onContinue: () => void }) {
  const auth = useAuth();
  const [showCallbackUrlInput, setShowCallbackUrlInput] = useState(false);
  const [didClickSignIn, setDidClickSignIn] = useState(false);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col items-start gap-2">
        <div className="flex items-center gap-3">
          <OnboardingButton
            onClick={() => {
              setDidClickSignIn(true);
              auth?.signIn();
            }}
          >
            Sign in
          </OnboardingButton>
          {didClickSignIn && (
            <button
              type="button"
              className="text-sm text-neutral-500 underline hover:text-neutral-600"
              onClick={() => setShowCallbackUrlInput(true)}
            >
              Something not working?
            </button>
          )}
        </div>
        <button
          type="button"
          onClick={() => {
            void analyticsCommands.event({ event: "onboarding_login_skipped" });
            onContinue();
          }}
          className="text-sm text-neutral-500/70 transition-colors hover:text-neutral-700"
        >
          Skip for now
        </button>
      </div>
      {showCallbackUrlInput && <CallbackUrlInput />}
    </div>
  );
}

function CallbackUrlInput() {
  const auth = useAuth();

  const [callbackUrl, setCallbackUrl] = useState("");

  return (
    <div className="relative flex items-center overflow-hidden rounded-full border border-neutral-200 transition-all duration-200 focus-within:border-neutral-400">
      <input
        type="text"
        className="flex-1 bg-white px-4 py-3 font-mono text-xs outline-hidden"
        placeholder="Paste browser url here, after you've signed in. (Like: http://char.com/callback/auth/?flow=desktop&scheme=hyprnote&access_token=<V>&refresh_token=<V>)"
        value={callbackUrl}
        onChange={(e) => setCallbackUrl(e.target.value)}
      />
      <button
        type="button"
        onClick={() => auth?.handleAuthCallback(callbackUrl)}
        disabled={!callbackUrl}
        className="absolute right-0.5 rounded-full bg-neutral-600 px-4 py-2 text-sm text-white transition-all enabled:hover:scale-[1.02] enabled:active:scale-[0.98] disabled:opacity-50"
      >
        Submit
      </button>
    </div>
  );
}
