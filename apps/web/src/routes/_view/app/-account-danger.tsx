import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

import { deleteAccount } from "@/functions/billing";
import { captureOperationalError } from "@/lib/error-reporting";

import { useAccountSession } from "./-account-session";
import { accountPillDangerClassName } from "./-account-ui";

export function DangerAreaSection() {
  const navigate = useNavigate();
  const account = useAccountSession();
  const [open, setOpen] = useState(false);
  const [confirmEmail, setConfirmEmail] = useState("");
  const email = account.data?.email;
  const emailConfirmed = confirmEmail.trim() === email;

  const deleteAccountMutation = useMutation({
    mutationFn: () => deleteAccount({ data: { email: confirmEmail } }),
    onSuccess: () => {
      setOpen(false);
      navigate({ to: "/" });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_delete",
      });
    },
  });

  const handleOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setConfirmEmail("");
      deleteAccountMutation.reset();
    }
  };

  return (
    <div
      className={cn([
        "overflow-hidden rounded-[24px] border border-red-200 bg-red-50",
        "shadow-[0_18px_50px_rgba(24,22,19,0.08)]",
        "p-6 sm:p-8",
      ])}
    >
      <p className="text-base font-medium text-red-900">Delete account</p>
      <p className="mt-3 text-sm leading-6 text-red-900">
        Anarlog is a local-first app. Your local notes, transcripts, and meeting
        data stay on your device. Deleting your account removes cloud-stored
        data.
      </p>

      <Dialog open={open} onOpenChange={handleOpenChange}>
        <button
          onClick={() => setOpen(true)}
          className={cn([accountPillDangerClassName, "mt-4"])}
        >
          Continue
        </button>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete your account?</DialogTitle>
            <DialogDescription>
              This cannot be undone. Your cloud-stored data is deleted within 30
              days, unless we are legally required to keep it. Local data on
              your devices stays there unless you remove it separately.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-2">
            <p className="text-sm leading-6 text-[#756b5d]">
              Type your email address to confirm.
            </p>
            <input
              type="email"
              value={confirmEmail}
              onChange={(e) => setConfirmEmail(e.target.value)}
              placeholder={email ?? "your@email.com"}
              autoComplete="off"
              className={cn([
                "h-10 w-full rounded-lg border border-neutral-300 bg-white px-3 text-sm",
                "focus:border-stone-500 focus:outline-none",
              ])}
            />
            {emailConfirmed && confirmEmail && (
              <p className="text-sm text-green-700">Email matches.</p>
            )}
            {deleteAccountMutation.isError && (
              <p className="text-sm text-red-600">
                {deleteAccountMutation.error?.message ||
                  "Failed to delete account"}
              </p>
            )}
          </div>
          <DialogFooter>
            <button
              onClick={() => handleOpenChange(false)}
              disabled={deleteAccountMutation.isPending}
              className={accountPillDangerClassName}
            >
              Cancel
            </button>
            <button
              onClick={() => deleteAccountMutation.mutate()}
              disabled={
                deleteAccountMutation.isPending || !emailConfirmed || !email
              }
              className={cn([
                "flex h-9 cursor-pointer items-center justify-center rounded-full bg-red-700 px-4",
                "text-sm font-medium text-white transition-colors hover:bg-red-800",
                "disabled:cursor-not-allowed disabled:opacity-50",
              ])}
            >
              {deleteAccountMutation.isPending
                ? "Deleting..."
                : "Delete account"}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
