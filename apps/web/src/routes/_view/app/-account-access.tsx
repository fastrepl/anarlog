import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { signOutEverywhereFn, signOutFn } from "@/functions/auth";
import { captureOperationalError } from "@/lib/error-reporting";
import { resetPrivateRouteAnalyticsIdentity } from "@/lib/private-route-analytics";

import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 40rem)": "row",
    },
    gap: "1rem",
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
  },
  style2: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#181613",
  },
  style3: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style4: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 40rem)": "row",
    },
    gap: "1rem",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#ede7dc",
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
  },
});
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
      navigate({
        to: "/",
      });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_sign_out",
      });
      navigate({
        to: "/",
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
      navigate({
        to: "/",
      });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_sign_out_everywhere",
      });
      navigate({
        to: "/",
      });
    },
  });
  return (
    <div {...stylex.props(accountStyles.card)}>
      <div {...stylex.props(styles.style1)}>
        <div>
          <p {...stylex.props(styles.style2)}>Sign out</p>
          <p {...stylex.props(styles.style3)}>
            End your current session on this device.
          </p>
        </div>
        <button
          onClick={() => signOut.mutate()}
          disabled={signOut.isPending || signOutEverywhere.isPending}
          {...stylex.props([accountStyles.pill, accountStyles.pillDanger])}
        >
          {signOut.isPending ? "Signing out..." : "Sign out"}
        </button>
      </div>

      <div {...stylex.props(styles.style4)}>
        <div>
          <p {...stylex.props(styles.style2)}>Sign out everywhere</p>
          <p {...stylex.props(styles.style3)}>
            End sessions on every device where you're signed in.
          </p>
        </div>
        <button
          onClick={() => signOutEverywhere.mutate()}
          disabled={signOut.isPending || signOutEverywhere.isPending}
          {...stylex.props([accountStyles.pill, accountStyles.pillDanger])}
        >
          {signOutEverywhere.isPending
            ? "Signing out..."
            : "Sign out everywhere"}
        </button>
      </div>
    </div>
  );
}
