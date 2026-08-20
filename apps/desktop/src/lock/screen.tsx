import { Trans, useLingui } from "@lingui/react/macro";
import { Lock } from "@phosphor-icons/react";
import { platform } from "@tauri-apps/plugin-os";

import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

export function LockScreen({
  title,
  description,
  authenticating,
  onUnlock,
}: {
  title: string;
  description?: string;
  authenticating: boolean;
  onUnlock: () => void;
}) {
  return (
    <div
      data-lock-screen
      data-tauri-drag-region
      className={cn([
        "bg-background flex h-full min-h-0 w-full flex-1 flex-col items-center justify-center",
      ])}
    >
      <div className="flex max-w-sm flex-col items-center px-6 text-center">
        <div className="bg-muted flex size-14 items-center justify-center rounded-full">
          <Lock className="text-foreground size-6" weight="fill" />
        </div>
        <h1 className="mt-5 text-lg font-semibold">{title}</h1>
        {description ? (
          <p className="text-muted-foreground mt-2 text-sm">{description}</p>
        ) : null}
        <Button
          className="mt-6"
          disabled={authenticating}
          onClick={onUnlock}
          data-tauri-drag-region="false"
        >
          {authenticating ? (
            <Trans>Authenticating…</Trans>
          ) : (
            <Trans>Unlock</Trans>
          )}
        </Button>
      </div>
    </div>
  );
}

export function useDeviceAuthHint() {
  const { t } = useLingui();
  switch (platform()) {
    case "macos":
      return t`Touch ID or enter your password to allow this.`;
    case "windows":
      return t`Use Windows Hello to allow this.`;
    default:
      return t`Authenticate to continue.`;
  }
}

export function NoteLockScreen({
  sessionTitle,
  authenticating,
  onUnlock,
}: {
  sessionTitle?: string;
  authenticating: boolean;
  onUnlock: () => void;
}) {
  const { t } = useLingui();
  const hint = useDeviceAuthHint();
  return (
    <LockScreen
      title={sessionTitle || t`Locked Note`}
      description={hint}
      authenticating={authenticating}
      onUnlock={onUnlock}
    />
  );
}
