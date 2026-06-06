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
    <div className="from-background via-card to-card relative flex h-full flex-col overflow-hidden bg-linear-to-b select-none">
      <div className="from-muted/40 pointer-events-none absolute inset-x-0 top-0 h-32 bg-linear-to-b to-transparent" />

      <div
        data-tauri-drag-region
        className="relative z-10 flex shrink-0 items-center px-3 pt-12"
      >
        <button
          type="button"
          onClick={onBack}
          className={cn([
            "text-muted-foreground hover:bg-muted/70 hover:text-muted-foreground flex h-9 items-center gap-1.5 rounded-full px-3 transition-colors",
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
          <div className="border-border/70 bg-card/90 flex h-14 w-14 items-center justify-center rounded-[20px] border shadow-[0_6px_18px_rgba(28,25,23,0.05)]">
            <img
              src="/assets/anarlog-icon.png"
              alt=""
              className="h-7 w-7 object-contain object-center"
            />
          </div>

          <div className="flex max-w-[17rem] flex-col gap-3">
            <div className="text-muted-foreground text-[10px] font-medium tracking-[0.22em] uppercase">
              Browser step required
            </div>
            <h2 className="text-foreground font-sans text-[22px] leading-[1.15] font-semibold sm:text-[28px]">
              {title}
            </h2>
            <p className="text-muted-foreground text-sm leading-6">
              {description}
            </p>
          </div>

          <div className="flex items-center gap-2.5 pt-1">
            <div className="bg-muted-foreground/75 h-1.5 w-1.5 rounded-full" />
            <div className="bg-muted h-1.5 w-1.5 rounded-full" />
            <div className="bg-muted h-1.5 w-1.5 rounded-full" />
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
              "bg-card text-muted-foreground hover:bg-background border-border h-10 w-full",
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
        description="Finish checkout in your browser, then return to Anarlog."
        actionLabel="Reopen checkout page"
        onBack={onBack}
        url={url}
      />
    );
  }

  return (
    <ExternalInstruction
      title="Connect your integration"
      description="Authorize access in your browser, then return to Anarlog."
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
      description="Complete sign-in in your browser, then return to Anarlog."
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
          <p className="text-muted-foreground text-xs leading-5">
            Paste the browser URL here if the browser button did not reopen
            Anarlog.
          </p>
        </>
      ) : (
        <button
          type="button"
          onClick={() => setShowCallbackInput(true)}
          className={cn([
            "text-muted-foreground hover:text-muted-foreground text-xs font-medium underline underline-offset-4 transition-colors",
          ])}
        >
          Browser handoff not working? Paste the callback link instead
        </button>
      )}
    </InstructionShell>
  );
}
