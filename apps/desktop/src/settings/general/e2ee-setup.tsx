import { Trans } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { CopyIcon, KeyRoundIcon, Loader2Icon } from "lucide-react";
import { useRef, useState } from "react";

import {
  createE2eeIdentity,
  importE2eeIdentity,
  inspectE2eeRecoveryKey,
} from "@hypr/plugin-db";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { Input } from "@hypr/ui/components/ui/input";

import { env } from "~/env";

const RECOVERY_KEY_CLIPBOARD_TTL_MS = 60_000;

async function clearRecoveryKeyClipboard(recoveryKey: string) {
  try {
    if ((await navigator.clipboard.readText()) === recoveryKey) {
      await navigator.clipboard.writeText("");
    }
  } catch {
    // Clipboard reads are not available on every platform.
  }
}

async function claimE2eeIdentity(accessToken: string, keyId: string) {
  const response = await fetch(
    new URL("/sync/e2ee/identity", env.VITE_API_URL),
    {
      method: "PUT",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ keyId }),
    },
  );
  if (response.status === 409) {
    throw new Error(
      "This account already uses another recovery key. Use the key from your first device.",
    );
  }
  if (!response.ok) {
    throw new Error("Could not protect this account. Try again.");
  }
  const identity = (await response.json()) as { keyId?: unknown };
  if (identity.keyId !== keyId) {
    throw new Error("The server returned an invalid key identity.");
  }
}

export function E2eeSetupDialog({
  open,
  onOpenChange,
  accountUserId,
  accessToken,
  onReady,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  accountUserId: string;
  accessToken: string;
  onReady: () => void;
}) {
  const [mode, setMode] = useState<"choose" | "import">("choose");
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const clipboardClearTimer = useRef<number | null>(null);
  const createMutation = useMutation({
    mutationFn: () => createE2eeIdentity(accountUserId),
    onSuccess: setRecoveryKey,
  });
  const importMutation = useMutation({
    mutationFn: async (recoveryKey: string) => {
      const identity = await inspectE2eeRecoveryKey(recoveryKey);
      await claimE2eeIdentity(accessToken, identity.keyId);
      await importE2eeIdentity(accountUserId, recoveryKey);
    },
    onSuccess: onReady,
  });
  const copyMutation = useMutation({
    mutationFn: async (recoveryKey: string) => {
      await navigator.clipboard.writeText(recoveryKey);
      if (clipboardClearTimer.current !== null) {
        window.clearTimeout(clipboardClearTimer.current);
      }
      clipboardClearTimer.current = window.setTimeout(() => {
        clipboardClearTimer.current = null;
        void clearRecoveryKeyClipboard(recoveryKey);
      }, RECOVERY_KEY_CLIPBOARD_TTL_MS);
    },
  });
  const importForm = useForm({
    defaultValues: { recoveryKey: "" },
    onSubmit: ({ value }) => importMutation.mutate(value.recoveryKey.trim()),
  });
  const error =
    createMutation.error ?? importMutation.error ?? copyMutation.error;
  const pending = createMutation.isPending || importMutation.isPending;

  const setOpen = (nextOpen: boolean) => {
    if (pending) return;
    if (!nextOpen) {
      setMode("choose");
      importForm.reset();
      setRecoveryKey(null);
      createMutation.reset();
      importMutation.reset();
    }
    onOpenChange(nextOpen);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[420px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
        <DialogHeader className="items-center gap-2 px-6 pt-6 text-center sm:text-center">
          <div className="bg-accent flex size-9 items-center justify-center rounded-full">
            <KeyRoundIcon className="size-4" aria-hidden="true" />
          </div>
          <DialogTitle className="text-foreground text-sm leading-5 font-semibold tracking-normal">
            <Trans>Protect cloud sync</Trans>
          </DialogTitle>
          <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.45]">
            <Trans>
              Your recovery key encrypts synced notes before they leave this
              device. Anarlog cannot read or recover it.
            </Trans>
          </DialogDescription>
        </DialogHeader>

        <div className="px-5 pt-5">
          {recoveryKey ? (
            <div className="space-y-3">
              <p className="text-muted-foreground text-center text-xs leading-5">
                <Trans>
                  Save this key in a password manager. You need it to add a new
                  device, and it will not be shown again.
                </Trans>
              </p>
              <code className="bg-muted block max-h-28 overflow-auto rounded-xl p-3 font-mono text-[11px] leading-5 break-all select-all">
                {recoveryKey}
              </code>
              <Button
                variant="outline"
                className="h-8 w-full rounded-full text-xs"
                onClick={() => copyMutation.mutate(recoveryKey)}
                disabled={copyMutation.isPending}
              >
                <CopyIcon className="size-3.5" aria-hidden="true" />
                <Trans>Copy recovery key</Trans>
              </Button>
              <p className="text-muted-foreground text-center text-[11px] leading-4">
                Clipboard copies clear after 60 seconds when supported.
              </p>
            </div>
          ) : mode === "import" ? (
            <div className="space-y-3">
              <p className="text-muted-foreground text-xs leading-5">
                <Trans>
                  Enter the recovery key from an existing Anarlog device.
                </Trans>
              </p>
              <importForm.Field name="recoveryKey">
                {(field) => (
                  <Input
                    aria-label="Recovery key"
                    value={field.state.value}
                    onChange={(event) => field.handleChange(event.target.value)}
                    placeholder="anarlog-e2ee-v1:..."
                    autoComplete="off"
                    spellCheck={false}
                    className="font-mono text-xs"
                  />
                )}
              </importForm.Field>
            </div>
          ) : (
            <div className="grid gap-2">
              <Button
                className="h-9 rounded-full text-xs"
                onClick={() => createMutation.mutate()}
                disabled={pending}
              >
                {createMutation.isPending && (
                  <Loader2Icon className="size-3.5 animate-spin" />
                )}
                <Trans>Create a recovery key</Trans>
              </Button>
              <Button
                variant="outline"
                className="h-9 rounded-full text-xs"
                onClick={() => setMode("import")}
                disabled={pending}
              >
                <Trans>Use an existing key</Trans>
              </Button>
            </div>
          )}

          {error && (
            <p className="mt-3 text-center text-xs text-red-500">
              {error.message}
            </p>
          )}
        </div>

        <DialogFooter className="grid grid-cols-2 gap-2 px-5 pt-5 pb-5 sm:grid-cols-2 sm:justify-normal">
          <Button
            variant="ghost"
            className="bg-accent/80 h-8 rounded-full px-4 text-xs font-medium shadow-none"
            onClick={() =>
              mode === "import" && !recoveryKey
                ? setMode("choose")
                : setOpen(false)
            }
            disabled={pending}
          >
            {mode === "import" && !recoveryKey ? (
              <Trans>Back</Trans>
            ) : (
              <Trans>Cancel</Trans>
            )}
          </Button>
          {recoveryKey ? (
            <Button
              className="h-8 rounded-full px-4 text-xs font-medium"
              onClick={() => importMutation.mutate(recoveryKey)}
              disabled={pending}
            >
              {importMutation.isPending && (
                <Loader2Icon className="size-3.5 animate-spin" />
              )}
              <Trans>I saved it</Trans>
            </Button>
          ) : mode === "import" ? (
            <importForm.Subscribe
              selector={(state) => state.values.recoveryKey}
            >
              {(importedKey) => (
                <Button
                  className="h-8 rounded-full px-4 text-xs font-medium"
                  onClick={() => void importForm.handleSubmit()}
                  disabled={!importedKey.trim() || pending}
                >
                  {importMutation.isPending && (
                    <Loader2Icon className="size-3.5 animate-spin" />
                  )}
                  <Trans>Unlock sync</Trans>
                </Button>
              )}
            </importForm.Subscribe>
          ) : (
            <span />
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
