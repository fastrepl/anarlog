import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { signOutFn } from "@/functions/auth";
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
          disabled={signOut.isPending}
          className={accountPillDangerClassName}
        >
          {signOut.isPending ? "Signing out..." : "Sign out"}
        </button>
      </div>
    </div>
  );
}
