import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy, DownloadSimple, Key } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { downloadDir, join } from "@tauri-apps/api/path";
import { useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import {
  createE2eeIdentity,
  importE2eeIdentity,
  inspectE2eeRecoveryKey,
} from "@anlg/plugin-db";
import { commands as fs2Commands } from "@anlg/plugin-fs2";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { Input } from "@anlg/ui/components/ui/input";

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
  const downloadMutation = useMutation({
    mutationFn: async (recoveryKey: string) => {
      const downloadsPath = await downloadDir();
      const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
      const path = await join(
        downloadsPath,
        `anarlog-recovery-key_${timestamp}.txt`,
      );
      const result = await fs2Commands.writeTextFile(path, `${recoveryKey}\n`);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return path;
    },
    onSuccess: (path) => void openerCommands.revealItemInDir(path),
  });
  const importForm = useForm({
    defaultValues: { recoveryKey: "" },
    onSubmit: ({ value }) => importMutation.mutate(value.recoveryKey.trim()),
  });
  const error =
    createMutation.error ??
    importMutation.error ??
    copyMutation.error ??
    downloadMutation.error;
  const pending = createMutation.isPending || importMutation.isPending;

  const setOpen = (nextOpen: boolean) => {
    if (pending) return;
    if (!nextOpen) {
      setMode("choose");
      importForm.reset();
      setRecoveryKey(null);
      createMutation.reset();
      importMutation.reset();
      copyMutation.reset();
      downloadMutation.reset();
    }
    onOpenChange(nextOpen);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        ref={(node) => {
          const closeButton = node?.querySelector<HTMLElement>(
            ":scope > button:last-child",
          );
          if (closeButton) closeButton.hidden = true;
        }}
        sx={styles.dialog}
      >
        <DialogHeader sx={styles.header}>
          <div {...stylex.props(styles.keyIconFrame)}>
            <Key {...stylex.props(styles.icon)} aria-hidden="true" />
          </div>
          <DialogTitle sx={styles.title}>
            <Trans>Protect cloud sync</Trans>
          </DialogTitle>
          <DialogDescription sx={styles.description}>
            <Trans>
              Your recovery key encrypts synced notes before they leave this
              device. Anarlog cannot read or recover it.
            </Trans>
          </DialogDescription>
        </DialogHeader>

        <div {...stylex.props(styles.body)}>
          {recoveryKey ? (
            <div {...stylex.props(styles.stack)}>
              <p {...stylex.props(styles.guidance)}>
                <Trans>
                  Save this key in a password manager. New devices can usually
                  be approved from another device; this key is your fallback and
                  will not be shown again.
                </Trans>
              </p>
              <code {...stylex.props(styles.recoveryKey)}>{recoveryKey}</code>
              <Button
                variant="outline"
                sx={styles.fullButton}
                onClick={() => copyMutation.mutate(recoveryKey)}
                disabled={copyMutation.isPending}
              >
                <Copy {...stylex.props(styles.smallIcon)} aria-hidden="true" />
                <Trans>Copy recovery key</Trans>
              </Button>
              <Button
                variant="outline"
                sx={styles.fullButton}
                onClick={() => downloadMutation.mutate(recoveryKey)}
                disabled={downloadMutation.isPending}
              >
                {downloadMutation.isPending ? (
                  <CircleNotch
                    {...stylex.props(styles.smallIcon, styles.spinning)}
                  />
                ) : (
                  <DownloadSimple
                    {...stylex.props(styles.smallIcon)}
                    aria-hidden="true"
                  />
                )}
                <Trans>Download recovery key (.txt)</Trans>
              </Button>
              <p {...stylex.props(styles.clipboardNote)}>
                Clipboard copies clear after 60 seconds when supported.
              </p>
            </div>
          ) : mode === "import" ? (
            <div {...stylex.props(styles.stack)}>
              <p {...stylex.props(styles.importGuidance)}>
                <Trans>
                  Enter your saved recovery key when another approved device is
                  unavailable.
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
                    sx={styles.recoveryInput}
                  />
                )}
              </importForm.Field>
            </div>
          ) : (
            <div {...stylex.props(styles.choiceGrid)}>
              <Button
                sx={styles.choiceButton}
                onClick={() => createMutation.mutate()}
                disabled={pending}
              >
                {createMutation.isPending && (
                  <CircleNotch
                    {...stylex.props(styles.smallIcon, styles.spinning)}
                  />
                )}
                <Trans>Create a recovery key</Trans>
              </Button>
              <Button
                variant="outline"
                sx={styles.choiceButton}
                onClick={() => setMode("import")}
                disabled={pending}
              >
                <Trans>Use an existing key</Trans>
              </Button>
              <Button
                variant="ghost"
                sx={styles.cancelChoice}
                onClick={() => setOpen(false)}
                disabled={pending}
              >
                <Trans>Cancel</Trans>
              </Button>
            </div>
          )}

          {error && <p {...stylex.props(styles.error)}>{error.message}</p>}
        </div>

        {(recoveryKey || mode === "import") && (
          <DialogFooter sx={styles.footer}>
            <Button
              variant="ghost"
              sx={styles.backButton}
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
                sx={styles.choiceButton}
                onClick={() => importMutation.mutate(recoveryKey)}
                disabled={pending}
              >
                {importMutation.isPending && (
                  <CircleNotch
                    {...stylex.props(styles.smallIcon, styles.spinning)}
                  />
                )}
                <Trans>I saved it</Trans>
              </Button>
            ) : (
              <importForm.Subscribe
                selector={(state) => state.values.recoveryKey}
              >
                {(importedKey) => (
                  <Button
                    sx={styles.choiceButton}
                    onClick={() => void importForm.handleSubmit()}
                    disabled={!importedKey.trim() || pending}
                  >
                    {importMutation.isPending && (
                      <CircleNotch
                        {...stylex.props(styles.smallIcon, styles.spinning)}
                      />
                    )}
                    <Trans>Unlock sync</Trans>
                  </Button>
                )}
              </importForm.Subscribe>
            )}
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  backButton: {
    backgroundColor: `color-mix(in srgb, ${colors.accent} 80%, transparent)`,
    borderRadius: radii.full,
    boxShadow: "none",
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  body: {
    paddingInline: "1rem",
    paddingTop: "1rem",
  },
  cancelChoice: {
    alignSelf: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": "transparent",
    },
    borderRadius: radii.full,
    boxShadow: "none",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    fontSize: "0.6875rem",
    fontWeight: 400,
    height: "1.5rem",
    justifySelf: "center",
    paddingInline: "0.75rem",
  },
  choiceButton: {
    borderRadius: radii.full,
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  choiceGrid: {
    display: "grid",
    gap: "0.5rem",
  },
  clipboardNote: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    lineHeight: "1rem",
    textAlign: "center",
  },
  description: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    lineHeight: 1.36,
    maxWidth: "16.25rem",
    textAlign: "center",
  },
  dialog: {
    backdropFilter: "blur(24px)",
    backgroundColor: `color-mix(in srgb, ${colors.card} 95%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 45%, transparent)`,
    borderRadius: "26px",
    boxShadow: "0 24px 70px rgb(0 0 0 / 0.32)",
    gap: 0,
    maxWidth: "20rem",
    overflow: "hidden",
    padding: 0,
    width: "calc(100vw - 48px)",
  },
  error: {
    color: "rgb(239 68 68)",
    fontSize: "0.75rem",
    marginTop: "0.75rem",
    textAlign: "center",
  },
  footer: {
    display: "grid",
    gap: "0.5rem",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    justifyContent: "normal",
    paddingBottom: "1rem",
    paddingInline: "1rem",
    paddingTop: "1rem",
  },
  fullButton: {
    borderRadius: radii.full,
    fontSize: "0.75rem",
    height: "2rem",
    width: "100%",
  },
  guidance: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
    textAlign: "center",
  },
  header: {
    alignItems: "center",
    gap: "0.5rem",
    paddingInline: "1.25rem",
    paddingTop: "1.75rem",
    textAlign: "center",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  importGuidance: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
  },
  keyIconFrame: {
    alignItems: "center",
    backgroundColor: colors.accent,
    borderRadius: radii.full,
    display: "flex",
    height: "2.25rem",
    justifyContent: "center",
    width: "2.25rem",
  },
  recoveryInput: {
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
  },
  recoveryKey: {
    backgroundColor: colors.muted,
    borderRadius: radii.xl,
    display: "block",
    fontFamily: fonts.mono,
    fontSize: "0.6875rem",
    lineHeight: "1.25rem",
    maxHeight: "7rem",
    overflow: "auto",
    overflowWrap: "anywhere",
    padding: "0.75rem",
    userSelect: "all",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  stack: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  title: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    fontWeight: 600,
    letterSpacing: "normal",
    lineHeight: "1.25rem",
  },
});

export { styles as e2eeSetupStyles };
