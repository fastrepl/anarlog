import { t } from "@lingui/core/macro";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { TrialDialogIcon } from "./trial-dialog-icon";

import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";

export function TrialPaymentReminderDialog({
  open,
  onOpenChange,
  daysRemaining,
  onAddPaymentMethod,
  onReferFriend,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  daysRemaining: number;
  onAddPaymentMethod: () => void;
  onReferFriend?: () => void;
}) {
  const title =
    daysRemaining === 1
      ? t`Your Pro trial ends in 1 day`
      : t`Your Pro trial ends in ${daysRemaining} days`;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent className="sm:max-w-[360px]">
        <DialogHeader className="items-center gap-2 text-center sm:text-center">
          <TrialDialogIcon state="started" />
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            {title}
          </DialogTitle>
          <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
            {t`Add a payment method before it ends to keep using Pro without an interruption.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 sm:grid-cols-2 sm:justify-normal">
          <GlassDialogCancelButton onClick={() => onOpenChange(false)}>
            {t`Not now`}
          </GlassDialogCancelButton>
          <Button
            className="bg-primary text-primary-foreground hover:bg-primary/90 h-8 rounded-full px-4 text-xs font-medium shadow-sm dark:bg-white dark:text-black dark:hover:bg-white/90"
            onClick={() => {
              onAddPaymentMethod();
              onOpenChange(false);
            }}
          >
            {t`Add payment method`}
          </Button>
        </DialogFooter>
        {onReferFriend && (
          <ReferralNudge
            daysRemaining={daysRemaining}
            onInvite={() => {
              onReferFriend();
              onOpenChange(false);
            }}
          />
        )}
      </GlassDialogContent>
    </Dialog>
  );
}

function ReferralNudge({
  daysRemaining,
  onInvite,
}: {
  daysRemaining: number;
  onInvite: () => void;
}) {
  useMountEffect(() => {
    void analyticsCommands
      .event({
        event: "referral_nudge_shown",
        surface: "desktop",
        reminder_threshold: daysRemaining <= 3 ? 3 : 7,
      })
      .catch(() => {});
  });
  return (
    <div className="border-border/60 flex flex-col items-center gap-2 border-t pt-4 text-center">
      <p className="text-foreground text-[13px] font-medium">
        {t`Need more time? Refer a friend.`}
      </p>
      <p className="text-muted-foreground text-xs leading-relaxed">
        {t`When they start their free trial, yours gets one more month. Up to 3 friends.`}
      </p>
      <Button
        variant="link"
        className="h-auto p-0 text-xs"
        onClick={() => {
          void analyticsCommands
            .event({
              event: "referral_nudge_clicked",
              surface: "desktop",
              reminder_threshold: daysRemaining <= 3 ? 3 : 7,
            })
            .catch(() => {});
          onInvite();
        }}
      >
        {t`Invite friends`}
      </Button>
    </div>
  );
}
