import { t } from "@lingui/core/macro";

import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

import { TrialDialogIcon } from "./trial-dialog-icon";
import { trialDialogStyles } from "./trial-dialog-styles";

import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";

export function TrialPaymentReminderDialog({
  open,
  onOpenChange,
  daysRemaining,
  onAddPaymentMethod,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  daysRemaining: number;
  onAddPaymentMethod: () => void;
}) {
  const title =
    daysRemaining === 1
      ? t`Your Pro trial ends in 1 day`
      : t`Your Pro trial ends in ${daysRemaining} days`;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader sx={trialDialogStyles.header}>
          <TrialDialogIcon state="started" />
          <DialogTitle sx={trialDialogStyles.title}>{title}</DialogTitle>
          <DialogDescription sx={trialDialogStyles.description}>
            {t`Add a payment method before it ends to keep using Pro without an interruption.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter sx={trialDialogStyles.footer}>
          <GlassDialogCancelButton onClick={() => onOpenChange(false)}>
            {t`Not now`}
          </GlassDialogCancelButton>
          <Button
            sx={trialDialogStyles.action}
            onClick={() => {
              onAddPaymentMethod();
              onOpenChange(false);
            }}
          >
            {t`Add payment method`}
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}
