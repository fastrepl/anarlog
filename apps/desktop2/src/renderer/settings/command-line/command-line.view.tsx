import { Trash2Icon, WrenchIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";

import type { EmbeddedCliStatus } from "../../../shared/embedded-cli";

interface Props {
  status: EmbeddedCliStatus | undefined;
  isLoading: boolean;
  isError: boolean;
  isBusy: boolean;
  installPending: boolean;
  uninstallPending: boolean;
  errorMessage: string | null;
  onInstall: () => void;
  onUninstall: () => void;
}

export function CommandLineView({
  status,
  isLoading,
  isError,
  isBusy,
  installPending,
  uninstallPending,
  errorMessage,
  onInstall,
  onUninstall,
}: Props) {
  return (
    <section className="flex flex-col gap-3">
      <div>
        <h2 className="mb-1 font-serif text-lg font-semibold">Command Line</h2>
        <p className="text-xs text-neutral-600">
          Install the embedded Char CLI as a shell command.
        </p>
      </div>

      {isError ? (
        <p className="text-xs text-red-600">Failed to load CLI status.</p>
      ) : isLoading || !status ? (
        <p className="text-xs text-neutral-500">Checking...</p>
      ) : (
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium">{status.commandName}</p>
            <p className="text-xs text-neutral-500">{status.installPath}</p>
          </div>

          {status.state === "installed" ? (
            <Button
              size="sm"
              type="button"
              variant="outline"
              onClick={onUninstall}
              disabled={isBusy}
            >
              <Trash2Icon className="mr-2 size-3.5" />
              {uninstallPending ? "Uninstalling..." : "Uninstall"}
            </Button>
          ) : (
            <Button
              size="sm"
              type="button"
              onClick={onInstall}
              disabled={
                isBusy ||
                status.state === "unsupported" ||
                status.state === "resource_missing"
              }
            >
              <WrenchIcon className="mr-2 size-3.5" />
              {installPending
                ? status.state === "conflict"
                  ? "Replacing..."
                  : "Installing..."
                : status.state === "conflict"
                  ? "Replace"
                  : "Install"}
            </Button>
          )}
        </div>
      )}

      {errorMessage ? (
        <p className="text-xs text-red-600">{errorMessage}</p>
      ) : null}
    </section>
  );
}
