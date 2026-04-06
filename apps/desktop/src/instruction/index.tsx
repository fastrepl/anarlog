import { ChevronLeft, ExternalLink } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { cn } from "@hypr/utils";

import { useAuth } from "~/auth";

export type InstructionType = "sign-in" | "billing" | "integration";

function useInstructionCleanup(onCleanup?: () => void) {
  const cleanupRef = useRef(onCleanup);

  useEffect(() => {
    cleanupRef.current = onCleanup;
  }, [onCleanup]);

  useEffect(() => {
    return () => {
      cleanupRef.current?.();
    };
  }, []);
}

function InstructionShell({
  title,
  description,
  onBack,
  action,
  children,
}: {
  title: string;
  description: string;
  onBack: () => void;
  action?: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <div className="relative flex h-full flex-col overflow-hidden bg-[linear-gradient(180deg,_rgba(250,250,249,0.92)_0%,_rgba(255,255,255,1)_24%,_rgba(255,255,255,1)_100%)] select-none">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-32 bg-linear-to-b from-stone-100/40 to-transparent" />

      <div
        data-tauri-drag-region
        className="relative z-10 flex shrink-0 items-center px-3 pt-12"
      >
        <button
          type="button"
          onClick={onBack}
          className={cn([
            "flex h-9 items-center gap-1.5 rounded-full px-3 text-stone-400 transition-colors hover:bg-stone-100/70 hover:text-stone-700",
          ])}
        >
          <ChevronLeft className="h-4 w-4" />
          <span className="text-xs font-medium">Back</span>
        </button>
      </div>

      <div
        data-tauri-drag-region
        className="relative z-10 flex flex-1 items-center justify-center p-6"
      >
        <div className="flex w-full max-w-sm flex-col items-center gap-6 px-10 pb-10 text-center">
          <div className="flex h-14 w-14 items-center justify-center rounded-[20px] border border-stone-200/70 bg-white/90 shadow-[0_6px_18px_rgba(28,25,23,0.05)]">
            <img
              src="/assets/char-logo-icon-black.svg"
              alt=""
              className="h-7 w-7"
            />
          </div>

          <div className="flex max-w-[17rem] flex-col gap-3">
            <div className="text-[10px] font-medium tracking-[0.22em] text-stone-400 uppercase">
              Browser step required
            </div>
            <h2 className="font-serif text-[22px] leading-[1.15] font-semibold text-stone-900 sm:text-[28px]">
              {title}
            </h2>
            <p className="text-sm leading-6 text-stone-500">{description}</p>
          </div>

          <div className="flex items-center gap-2.5 pt-1">
            <div className="h-1.5 w-1.5 rounded-full bg-stone-400/75" />
            <div className="h-1.5 w-1.5 rounded-full bg-stone-300" />
            <div className="h-1.5 w-1.5 rounded-full bg-stone-300" />
          </div>

          {action ? <div className="w-full">{action}</div> : null}
          {children ? (
            <div className="flex w-full flex-col items-center gap-3">
              {children}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function ExternalInstruction({
  title,
  description,
  actionLabel,
  onBack,
  url,
}: {
  title: string;
  description: string;
  actionLabel: string;
  onBack: () => void;
  url?: string;
}) {
  return (
    <InstructionShell
      title={title}
      description={description}
      onBack={onBack}
      action={
        url ? (
          <Button
            variant="outline"
            className={cn([
              "h-10 w-full border-stone-300 bg-white text-stone-700 hover:bg-stone-50",
            ])}
            onClick={() => void openerCommands.openUrl(url, null)}
          >
            {actionLabel}
            <ExternalLink className="size-3.5" />
          </Button>
        ) : undefined
      }
    />
  );
}

export function InstructionScreen({
  type,
  onBack,
  url,
  onCleanup,
}: {
  type: InstructionType;
  onBack: () => void;
  url?: string;
  onCleanup?: () => void;
}) {
  useInstructionCleanup(onCleanup);

  if (type === "sign-in") {
    return <SignInInstruction onBack={onBack} />;
  }

  if (type === "billing") {
    return (
      <ExternalInstruction
        title="Complete your purchase"
        description="Finish checkout in your browser, then return to Char."
        actionLabel="Reopen checkout page"
        onBack={onBack}
        url={url}
      />
    );
  }

  return (
    <ExternalInstruction
      title="Connect your integration"
      description="Authorize access in your browser, then return to Char."
      actionLabel="Reopen in browser"
      onBack={onBack}
      url={url}
    />
  );
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

  return (
    <InstructionShell
      title="Sign in to your account"
      description="Complete sign-in in your browser, then return to Char."
      onBack={onBack}
    >
      {showCallbackInput ? (
        <>
          <div className="flex w-full flex-col gap-2">
            <Input
              type="text"
              className="h-10 font-mono text-xs"
              placeholder="hyprnote://deeplink/auth?access_token=..."
              value={callbackUrl}
              onChange={(e) => setCallbackUrl(e.target.value)}
            />
            <Button
              className="h-10"
              onClick={() => void auth.handleAuthCallback(callbackUrl)}
              disabled={!callbackUrl}
            >
              Submit callback URL
            </Button>
          </div>
          <p className="text-xs leading-5 text-neutral-500">
            Paste the browser URL here if the browser button did not reopen
            Char.
          </p>
        </>
      ) : (
        <button
          type="button"
          onClick={() => setShowCallbackInput(true)}
          className={cn([
            "text-xs font-medium text-neutral-500 underline underline-offset-4 transition-colors hover:text-neutral-700",
          ])}
        >
          Browser handoff not working? Paste the callback link instead
        </button>
      )}
    </InstructionShell>
  );
}
