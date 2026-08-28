import { ShieldCheck } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";

import { AuthShell, authStyles } from "@/components/auth-shell";
import { exchangeOtpToken } from "@/functions/auth";
import {
  DEFAULT_DESKTOP_SCHEME,
  desktopSchemeSchema,
} from "@/functions/desktop-flow";
import {
  resolveAuthFlowContext,
  toAuthFlowSearch,
} from "@/lib/auth-flow-context";
import { buildPostAuthDestination } from "@/lib/auth-redirect";
import { identifyPrivateRouteUser } from "@/lib/private-route-analytics";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style2: {
    marginInline: "auto",
    marginBottom: ".5rem",
    width: "1.25rem",
    height: "1.25rem",
    color: "#4f4940",
  },
  style3: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style4: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#b91c1c",
  },
  style5: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    textDecorationLine: {
      default: null,
      ":hover": "underline",
    },
  },
});
const validateSearch = z.object({
  token_hash: z.string().min(1),
  type: z.enum([
    "email",
    "recovery",
    "magiclink",
    "signup",
    "invite",
    "email_change",
  ]),
  flow: z.enum(["desktop", "web"]).optional(),
  scheme: desktopSchemeSchema.optional(),
  redirect: z.string().optional(),
  redirect_to: z.string().max(2048).optional(),
});
export const Route = createFileRoute("/confirm-auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
  }),
});
function Component() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const [errorMessage, setErrorMessage] = useState("");
  const [noticeMessage, setNoticeMessage] = useState("");
  const context = resolveAuthFlowContext({
    flow: search.flow,
    scheme: search.scheme,
    redirect: search.redirect,
    redirectTo: search.redirect_to,
  });
  const confirmMutation = useMutation({
    mutationFn: () =>
      exchangeOtpToken({
        data: {
          token_hash: search.token_hash,
          type: search.type,
          flow: context.flow,
        },
      }),
    onSuccess: (result) => {
      if (!result.success) {
        if ("pendingEmailChange" in result && result.pendingEmailChange) {
          setNoticeMessage(
            "This link is confirmed. To finish changing your email, click the link in the email sent to your other address.",
          );
          return;
        }
        setErrorMessage(result.error);
        return;
      }
      identifyPrivateRouteUser(result.userId, {
        method: "otp",
        action: search.type,
        flow: context.flow,
      });
      if (search.type === "recovery") {
        navigate({
          to: "/update-password/",
          search: toAuthFlowSearch(context),
        });
        return;
      }
      if (context.flow === "web") {
        window.location.href = buildPostAuthDestination({
          newAccount: result.newAccount,
          returnTo: context.redirect,
        });
        return;
      }
      const params = new URLSearchParams({
        flow: "desktop",
        scheme: context.scheme ?? DEFAULT_DESKTOP_SCHEME,
        access_token: result.access_token,
        refresh_token: result.refresh_token,
      });
      window.location.href = `/callback/auth?${params.toString()}`;
    },
  });
  return (
    <AuthShell
      title="Continue securely"
      description="Confirm this action to continue with your account."
    >
      <div {...stylex.props(styles.style1)}>
        <div {...stylex.props(authStyles.notice)}>
          <ShieldCheck {...stylex.props(styles.style2)} />
          <p {...stylex.props(styles.style3)}>
            This extra step keeps automated email scanners from using your
            one-time link.
          </p>
        </div>

        {errorMessage && <p {...stylex.props(styles.style4)}>{errorMessage}</p>}

        {noticeMessage && (
          <p {...stylex.props(styles.style3)}>{noticeMessage}</p>
        )}

        {!noticeMessage && (
          <button
            type="button"
            onClick={() => {
              setErrorMessage("");
              confirmMutation.mutate();
            }}
            disabled={confirmMutation.isPending}
            {...stylex.props(authStyles.primaryButton)}
          >
            {confirmMutation.isPending ? "Confirming..." : "Continue"}
          </button>
        )}

        <Link
          to="/auth/"
          search={toAuthFlowSearch(context)}
          {...stylex.props(styles.style5)}
        >
          Back to sign in
        </Link>
      </div>
    </AuthShell>
  );
}
