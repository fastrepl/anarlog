import { createFileRoute } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { dismissInstruction } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";

import { useAuth } from "~/auth";

type InstructionType = "sign-in" | "billing" | "integration";

export const Route = createFileRoute("/app/instruction")({
  validateSearch: (search): { type: InstructionType; url?: string } => ({
    type: ((search as { type?: string }).type ?? "sign-in") as InstructionType,
    url: (search as { url?: string }).url,
  }),
  component: InstructionRoute,
});

function useHandleBack() {
  return useCallback(() => dismissInstruction(), []);
}

function SignInInstruction({ onBack }: { onBack: () => void }) {
  const auth = useAuth();
  const [callbackUrl, setCallbackUrl] = useState("");
  const [showCallbackInput, setShowCallbackInput] = useState(false);

  useEffect(() => {
    if (!auth?.session) {
      return;
    }

    onBack();
  }, [auth?.session, onBack]);

  const handleManualCallback = useCallback(async () => {
    if (!callbackUrl) {
      return;
    }

    await auth?.handleAuthCallback(callbackUrl);
  }, [auth, callbackUrl]);

  return (
    <InstructionLayout
      title="Sign in to your account"
      description="Complete sign-in in your browser, then return to Char."
      onBack={onBack}
    >
      <div className="flex w-full max-w-[260px] flex-col items-center gap-3">
        {showCallbackInput ? (
          <>
            <div className="flex w-full flex-col gap-2">
              <Input
                type="text"
                className="font-mono text-xs"
                placeholder="hyprnote://deeplink/auth?access_token=..."
                value={callbackUrl}
                onChange={(e) => setCallbackUrl(e.target.value)}
              />
              <Button
                onClick={() => void handleManualCallback()}
                disabled={!callbackUrl}
              >
                Submit
              </Button>
            </div>
            <p className="text-center text-xs text-neutral-500">
              Paste the browser URL here if the button in your browser did not
              reopen Char.
            </p>
          </>
        ) : (
          <button
            type="button"
            onClick={() => setShowCallbackInput(true)}
            className="text-xs text-neutral-500 underline underline-offset-2 hover:text-neutral-700"
          >
            Button not working? Paste the link instead
          </button>
        )}
      </div>
    </InstructionLayout>
  );
}

function BillingInstruction({
  onBack,
  url,
}: {
  onBack: () => void;
  url?: string;
}) {
  return (
    <InstructionLayout
      title="Complete your purchase"
      description="Finish checkout in your browser, then return to Char."
      onBack={onBack}
    >
      {url && (
        <Button
          variant="outline"
          onClick={() => void openerCommands.openUrl(url, null)}
        >
          Reopen checkout page
        </Button>
      )}
    </InstructionLayout>
  );
}

function IntegrationInstruction({
  onBack,
  url,
}: {
  onBack: () => void;
  url?: string;
}) {
  return (
    <InstructionLayout
      title="Connect your integration"
      description="Authorize access in your browser, then return to Char."
      onBack={onBack}
    >
      {url && (
        <Button
          variant="outline"
          onClick={() => void openerCommands.openUrl(url, null)}
        >
          Reopen in browser
        </Button>
      )}
    </InstructionLayout>
  );
}

function InstructionLayout({
  title,
  description,
  onBack,
  children,
}: {
  title: string;
  description: string;
  onBack: () => void;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex h-full flex-col select-none">
      <div
        data-tauri-drag-region
        className="flex shrink-0 items-center px-3 pt-12"
      >
        <button
          type="button"
          onClick={onBack}
          className="flex h-8 items-center gap-1 rounded-lg px-2 text-neutral-400 transition-colors hover:bg-neutral-100 hover:text-neutral-600"
        >
          <ChevronLeft className="h-4 w-4" />
          <span className="text-xs">Go back</span>
        </button>
      </div>

      <div
        data-tauri-drag-region
        className="flex flex-1 flex-col items-center justify-center gap-6 p-8"
      >
        <img
          src="/assets/char-logo-icon-black.svg"
          alt=""
          className="h-10 w-10"
        />

        <div className="flex flex-col items-center gap-2 text-center">
          <h2 className="font-serif text-lg font-semibold">{title}</h2>
          <p className="text-sm text-neutral-500">{description}</p>
        </div>

        <div className="flex items-center gap-2">
          <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400 [animation-delay:-0.3s]" />
          <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400 [animation-delay:-0.15s]" />
          <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400" />
        </div>

        {children}
      </div>
    </div>
  );
}

function InstructionRoute() {
  const { type, url } = Route.useSearch();
  const handleBack = useHandleBack();
  const onBack = useCallback(() => void handleBack(), [handleBack]);

  switch (type) {
    case "sign-in":
      return <SignInInstruction onBack={onBack} />;
    case "billing":
      return <BillingInstruction onBack={onBack} url={url} />;
    case "integration":
      return <IntegrationInstruction onBack={onBack} url={url} />;
  }
}
