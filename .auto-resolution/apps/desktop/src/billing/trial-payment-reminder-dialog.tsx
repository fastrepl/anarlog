import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";

import { TrialDialogIcon } from "./trial-dialog-icon";

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
  const dayLabel = daysRemaining === 1 ? "day" : "days";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
        <DialogHeader className="items-center gap-2 px-5 pt-7 text-center sm:text-center">
          <TrialDialogIcon state="started" />
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            Your Pro trial ends in {daysRemaining} {dayLabel}
          </DialogTitle>
          <DialogDescription className="text-foreground max-w-[260px] text-center text-[13px] leading-[1.36]">
            Add a payment method before it ends to keep using Pro without an
            interruption.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 px-4 pt-4 pb-4 sm:grid-cols-2 sm:justify-normal">
          <Button
            variant="ghost"
            className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-4 text-xs font-medium shadow-none"
            onClick={() => onOpenChange(false)}
          >
            Not now
          </Button>
          <Button
            className="bg-primary text-primary-foreground hover:bg-primary/90 h-8 rounded-full px-4 text-xs font-medium shadow-sm dark:bg-white dark:text-black dark:hover:bg-white/90"
            onClick={() => {
              onAddPaymentMethod();
              onOpenChange(false);
            }}
          >
            Add payment method
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
