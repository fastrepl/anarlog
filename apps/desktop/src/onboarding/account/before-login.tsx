import { useEffect, useState } from "react";

import { OnboardingButton } from "../shared";

import { useAuth } from "~/auth";

export function BeforeLogin() {
  const auth = useAuth();
  const autoSignInCompleted = useAutoTriggerSignin();
  const [showCallbackUrlInput, setShowCallbackUrlInput] = useState(false);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3">
        <OnboardingButton
          onClick={() => auth?.signIn()}
          disabled={!autoSignInCompleted}
        >
          {autoSignInCompleted
            ? "Click here to Sign in"
            : "Signing in on your browser..."}
        </OnboardingButton>
        {autoSignInCompleted && (
          <button
            className="text-sm text-neutral-500 underline hover:text-neutral-600"
            onClick={() => setShowCallbackUrlInput(true)}
          >
            Something not working?
          </button>
        )}
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
        onClick={() => auth?.handleAuthCallback(callbackUrl)}
        disabled={!callbackUrl}
        className="absolute right-0.5 rounded-full bg-neutral-600 px-4 py-2 text-sm text-white transition-all enabled:hover:scale-[1.02] enabled:active:scale-[0.98] disabled:opacity-50"
      >
        Submit
      </button>
    </div>
  );
}

function useAutoTriggerSignin() {
  const auth = useAuth();
  const [triggered, setTriggered] = useState(false);

  useEffect(() => {
    if (!triggered && auth) {
      setTriggered(true);
      auth.signIn();
    }
  }, [auth, triggered]);

  return triggered;
}
