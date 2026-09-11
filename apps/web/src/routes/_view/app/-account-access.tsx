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

import { signOutEverywhereFn, signOutFn } from "@/functions/auth";
import { captureOperationalError } from "@/lib/error-reporting";
import { resetPrivateRouteAnalyticsIdentity } from "@/lib/private-route-analytics";

import {
  accountCardClassName,
  accountPillDangerClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

type ConfirmAction = "sign-out" | "sign-out-everywhere" | null;

export function AccountAccessSection() {
  const navigate = useNavigate();
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>(null);

  const signOut = useMutation({
    mutationFn: async () => {
      const res = await signOutFn();
      if (res.success) {
        return true;
      }

      throw new Error(res.message);
    },
    onSuccess: () => {
      resetPrivateRouteAnalyticsIdentity();
      setConfirmAction(null);
      navigate({ to: "/" });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_sign_out",
      });
    },
  });

  const signOutEverywhere = useMutation({
    mutationFn: async () => {
      const res = await signOutEverywhereFn();
      if (res.success) {
        return true;
      }

      throw new Error(res.message);
    },
    onSuccess: () => {
      resetPrivateRouteAnalyticsIdentity();
      setConfirmAction(null);
      navigate({ to: "/" });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_sign_out_everywhere",
      });
    },
  });

  const open = confirmAction != null;
  const isEverywhere = confirmAction === "sign-out-everywhere";
  const pending = signOut.isPending || signOutEverywhere.isPending;
  const error = isEverywhere ? signOutEverywhere.error : signOut.error;
  const errorMessage = error?.message || "Could not sign out. Try again.";

  const handleConfirm = () => {
    if (confirmAction === "sign-out-everywhere") {
      signOutEverywhere.mutate();
    } else if (confirmAction === "sign-out") {
      signOut.mutate();
    }
  };

  return (
    <div className={accountCardClassName}>
      <div className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:p-8">
        <div>
          <p className="text-base font-medium text-[#181613]">Sign out</p>
          <p className="mt-1 text-sm leading-6 text-[#756b5d]">
            End your current session on this device.
          </p>
        </div>
        <button
          onClick={() => setConfirmAction("sign-out")}
          disabled={pending}
          className={accountPillDangerClassName}
        >
          {signOut.isPending ? "Signing out..." : "Sign out"}
        </button>
      </div>

      <div className="flex flex-col gap-4 border-t border-[#ede7dc] p-6 sm:flex-row sm:items-center sm:justify-between sm:p-8">
        <div>
          <p className="text-base font-medium text-[#181613]">
            Sign out everywhere
          </p>
          <p className="mt-1 text-sm leading-6 text-[#756b5d]">
            End sessions on every device where you're signed in.
          </p>
        </div>
        <button
          onClick={() => setConfirmAction("sign-out-everywhere")}
          disabled={pending}
          className={accountPillDangerClassName}
        >
          {signOutEverywhere.isPending
            ? "Signing out..."
            : "Sign out everywhere"}
        </button>
      </div>

      <Dialog
        open={open}
        onOpenChange={(next) => !next && setConfirmAction(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {isEverywhere ? "Sign out everywhere?" : "Sign out?"}
            </DialogTitle>
            <DialogDescription>
              {isEverywhere
                ? "This will end every session on every device where you're signed in, including this one."
                : "This will end your session on this device."}
            </DialogDescription>
          </DialogHeader>
          {error && (
            <p className="text-sm text-red-600" role="alert">
              {errorMessage}
            </p>
          )}
          <DialogFooter>
            <button
              onClick={() => setConfirmAction(null)}
              disabled={pending}
              className={accountPillSecondaryClassName}
            >
              Cancel
            </button>
            <button
              onClick={handleConfirm}
              disabled={pending}
              className={cn([
                accountPillDangerClassName,
                "bg-red-700 text-white hover:bg-red-800",
              ])}
            >
              {pending ? "Signing out..." : "Sign out"}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
