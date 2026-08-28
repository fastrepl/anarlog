import { ShieldCheck } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";

import {
  AuthShell,
  authNoticeClassName,
  authPrimaryButtonClassName,
} from "@/components/auth-shell";
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
    meta: [{ name: "robots", content: "noindex, nofollow" }],
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
      <div className="flex flex-col gap-4">
        <div className={authNoticeClassName}>
          <ShieldCheck className="mx-auto mb-2 size-5 text-[#4f4940]" />
          <p className="text-center text-sm leading-6 text-[#756b5d]">
            This extra step keeps automated email scanners from using your
            one-time link.
          </p>
        </div>

        {errorMessage && (
          <p className="text-center text-sm leading-6 text-red-700">
            {errorMessage}
          </p>
        )}

        {noticeMessage && (
          <p className="text-center text-sm leading-6 text-[#756b5d]">
            {noticeMessage}
          </p>
        )}

        {!noticeMessage && (
          <button
            type="button"
            onClick={() => {
              setErrorMessage("");
              confirmMutation.mutate();
            }}
            disabled={confirmMutation.isPending}
            className={authPrimaryButtonClassName}
          >
            {confirmMutation.isPending ? "Confirming..." : "Continue"}
          </button>
        )}

        <Link
          to="/auth/"
          search={toAuthFlowSearch(context)}
          className="text-center text-sm text-[#756b5d] transition-colors hover:text-[#181613] hover:underline"
        >
          Back to sign in
        </Link>
      </div>
    </AuthShell>
  );
}
