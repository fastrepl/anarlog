import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { signOutEverywhereFn, signOutFn } from "@/functions/auth";
import { captureOperationalError } from "@/lib/error-reporting";
import { resetPrivateRouteAnalyticsIdentity } from "@/lib/private-route-analytics";

import {
  accountCardClassName,
  accountPillDangerClassName,
} from "./-account-ui";

export function AccountAccessSection() {
  const navigate = useNavigate();

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
      navigate({ to: "/" });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_sign_out",
      });
      navigate({ to: "/" });
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
      navigate({ to: "/" });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_sign_out_everywhere",
      });
      navigate({ to: "/" });
    },
  });

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
          onClick={() => signOut.mutate()}
          disabled={signOut.isPending || signOutEverywhere.isPending}
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
          onClick={() => signOutEverywhere.mutate()}
          disabled={signOut.isPending || signOutEverywhere.isPending}
          className={accountPillDangerClassName}
        >
          {signOutEverywhere.isPending
            ? "Signing out..."
            : "Sign out everywhere"}
        </button>
      </div>
    </div>
  );
}
