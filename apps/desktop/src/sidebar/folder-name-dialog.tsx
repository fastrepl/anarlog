import { Trans, useLingui } from "@lingui/react/macro";
import { useEffect, useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { Input } from "@anlg/ui/components/ui/input";

import { normalizeFolderPath } from "~/session/folders";
import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";

export function FolderNameDialog({
  open,
  title,
  confirmLabel,
  initialValue = "",
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  title: string;
  confirmLabel: string;
  initialValue?: string;
  onOpenChange: (open: boolean) => void;
  onSubmit: (folderPath: string) => Promise<void>;
}) {
  const { t } = useLingui();
  const [value, setValue] = useState(initialValue);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setValue(initialValue);
      setError(null);
      setBusy(false);
    }
  }, [initialValue, open]);

  const submit = async () => {
    const normalized = normalizeFolderPath(value.trim());
    if (!normalized || normalized.includes("/")) {
      setError(t`Enter a valid folder name.`);
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await onSubmit(normalized);
      onOpenChange(false);
    } catch (cause) {
      setError(
        String(cause instanceof Error ? cause.message : cause).includes(
          "folder_target_exists",
        )
          ? t`A folder with this name already exists.`
          : t`Could not save the folder.`,
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader className="items-center gap-2 text-center sm:text-center">
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            {title}
          </DialogTitle>
          <DialogDescription className="sr-only">
            <Trans>Folder name</Trans>
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <Input
            autoFocus
            value={value}
            disabled={busy}
            aria-label={t`Folder name`}
            onChange={(event) => {
              setValue(event.target.value);
              setError(null);
            }}
          />
          {error ? (
            <p className="text-destructive pt-2 text-center text-xs">{error}</p>
          ) : null}
          <DialogFooter className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-2 sm:justify-normal">
            <GlassDialogCancelButton
              disabled={busy}
              onClick={() => onOpenChange(false)}
            >
              <Trans>Cancel</Trans>
            </GlassDialogCancelButton>
            <Button
              type="submit"
              className="h-8 rounded-full px-4 text-xs font-medium shadow-sm"
              disabled={busy}
            >
              {confirmLabel}
            </Button>
          </DialogFooter>
        </form>
      </GlassDialogContent>
    </Dialog>
  );
}
