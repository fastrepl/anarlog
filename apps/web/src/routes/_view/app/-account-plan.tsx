import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";

import { getFixedPlanPrice } from "@anlg/pricing";
import { cn } from "@anlg/utils";

import { authInputClassName } from "@/components/auth-shell";
import { getAccountSubscription } from "@/functions/billing";
import { getSupabaseBrowserClient } from "@/functions/supabase";
import { applyYcPerk } from "@/functions/yc-perk";
import {
  accountWorkspacePlanQueryKey,
  fetchWorkspacePlan,
  getAccountPlanCopy,
} from "@/lib/account-plan";
import { validateYcPerkApplyValue } from "@/lib/yc-perk";

import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillPrimaryClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

export const accountSubscriptionQueryKey = ["account-subscription"];

const proPrice = getFixedPlanPrice("pro");

const ycPerkApplyErrorMessages = {
  claimed: "This perk has already been claimed.",
  invalid: "This YC code is not valid.",
  not_verified: "This YC link is no longer active.",
  email_missing: "Update your YC link to include your email.",
};

export function PlanSection({
  perk,
}: {
  perk?: "applied" | "claimed" | "invalid";
}) {
  const { data, isPending } = useAccountSession();
  const billing = data?.billing;
  const workspacePlanQuery = useQuery({
    queryKey: accountWorkspacePlanQueryKey,
    enabled: typeof window !== "undefined",
    retry: 1,
    queryFn: async ({ signal }) => {
      const supabase = getSupabaseBrowserClient();
      const { data: sessionData, error } = await supabase.auth.getSession();
      const session = sessionData.session;
      if (error || !session) {
        throw new Error("Sign in again to refresh your plan.");
      }
      return fetchWorkspacePlan({
        client: supabase,
        accessToken: session.access_token,
        signal,
      });
    },
  });
  const subscriptionQuery = useQuery({
    queryKey: accountSubscriptionQueryKey,
    enabled:
      typeof window !== "undefined" &&
      workspacePlanQuery.isSuccess &&
      workspacePlanQuery.data == null &&
      (billing?.isPaid === true ||
        billing?.isTrialing === true ||
        billing?.isPaused === true),
    queryFn: () => getAccountSubscription(),
  });

  const cancelAtPeriodEnd =
    subscriptionQuery.data?.cancelAtPeriodEnd ??
    billing?.cancelAtPeriodEnd ??
    false;
  const currentPeriodEnd =
    subscriptionQuery.data?.currentPeriodEnd != null
      ? new Date(subscriptionQuery.data.currentPeriodEnd * 1000)
      : (billing?.currentPeriodEnd ?? null);
  const hasYcPerk =
    subscriptionQuery.data?.hasYcPerk === true || perk === "applied";
  const currentPeriod = subscriptionQuery.data?.period ?? null;
  const switchTargetPeriod =
    billing?.isPaid === true &&
    billing.isTrialing !== true &&
    billing.isPaused !== true &&
    !cancelAtPeriodEnd &&
    proPrice?.yearly != null
      ? currentPeriod === "monthly"
        ? ("yearly" as const)
        : currentPeriod === "yearly"
          ? ("monthly" as const)
          : null
      : null;
  const workspacePlan = workspacePlanQuery.data ?? null;
  const isWorkspacePlan = workspacePlan != null;

  const { planLabel, planDetail } = getAccountPlanCopy({
    isTrialing: billing?.isTrialing === true,
    isPaused: billing?.isPaused === true,
    isPaid: billing?.isPaid === true,
    isLite: billing?.isLite,
    isPro: billing?.isPro,
    trialDaysRemaining: billing?.trialDaysRemaining ?? null,
    trialEnd: billing?.trialEnd ?? null,
    cancelAtPeriodEnd,
    currentPeriodEnd,
    hasYcPerk,
    workspacePlan,
  });

  const isCheckingPlan =
    isPending ||
    workspacePlanQuery.isPending ||
    (workspacePlanQuery.isSuccess &&
      workspacePlanQuery.data == null &&
      billing?.isPaid === true &&
      billing.isTrialing !== true &&
      subscriptionQuery.isPending);
  const couldNotVerifyPlan =
    !isCheckingPlan &&
    (workspacePlanQuery.isError || !workspacePlanQuery.isSuccess);

  return (
    <div className={accountCardClassName}>
      <div className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:p-8">
        {isCheckingPlan ? (
          <p className="text-sm leading-6 text-[#756b5d]">
            Checking your plan...
          </p>
        ) : couldNotVerifyPlan ? (
          <p className="text-sm leading-6 text-[#756b5d]">
            Couldn't verify your plan. Refresh to try again.
          </p>
        ) : (
          <>
            <div>
              <p className="text-base font-medium text-[#181613]">
                You're on{" "}
                <mark className="bg-[#fff0b3] px-1 font-semibold">
                  {planLabel}
                </mark>
              </p>
              <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                {planDetail}
              </p>
            </div>
            {isWorkspacePlan ? null : billing?.isPaid || billing?.isTrialing ? (
              <div className="flex flex-wrap items-center gap-2">
                {switchTargetPeriod ? (
                  <Link
                    to="/app/switch-plan/"
                    search={{ targetPeriod: switchTargetPeriod }}
                    className={accountPillSecondaryClassName}
                  >
                    {switchTargetPeriod === "yearly"
                      ? `Switch to yearly · $${proPrice?.yearly}/yr`
                      : `Switch to monthly · $${proPrice?.monthly}/mo`}
                  </Link>
                ) : null}
                <Link
                  to="/app/portal/"
                  className={accountPillSecondaryClassName}
                >
                  Manage billing
                </Link>
              </div>
            ) : billing?.isPaused ? (
              <Link
                to="/app/checkout/"
                search={{ trial: "false", source: "settings" }}
                className={accountPillPrimaryClassName}
              >
                Resume Pro
              </Link>
            ) : (
              <div className="flex flex-wrap items-center gap-2">
                <Link
                  to="/app/checkout/"
                  search={{
                    period: "monthly",
                    trial: "false",
                    source: "settings",
                  }}
                  className={accountPillPrimaryClassName}
                >
                  {proPrice
                    ? `Upgrade to Pro · $${proPrice.monthly}/mo`
                    : "Upgrade to Pro"}
                </Link>
                {proPrice?.yearly != null ? (
                  <Link
                    to="/app/checkout/"
                    search={{
                      period: "yearly",
                      trial: "false",
                      source: "settings",
                    }}
                    className={accountPillSecondaryClassName}
                  >
                    {`Pay yearly · $${proPrice.yearly}/yr`}
                  </Link>
                ) : null}
              </div>
            )}
          </>
        )}
      </div>
      {!isCheckingPlan &&
      !couldNotVerifyPlan &&
      !isWorkspacePlan &&
      !hasYcPerk ? (
        <YcPerkApplyForm perk={perk} />
      ) : null}
    </div>
  );
}

function YcPerkApplyForm({
  perk,
}: {
  perk?: "applied" | "claimed" | "invalid";
}) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const applyMutation = useMutation({
    mutationFn: (value: string) => applyYcPerk({ data: { value } }),
    onSuccess: (result) => {
      if (result.status === "needs_checkout" && result.code) {
        void navigate({
          to: "/app/checkout/",
          search: {
            period: "monthly",
            trial: "false",
            source: "yc_perk",
            code: result.code,
          },
        });
        return;
      }
      if (result.status === "applied" || result.status === "already_applied") {
        void queryClient.invalidateQueries({
          queryKey: accountSubscriptionQueryKey,
        });
      }
    },
  });
  const form = useForm({
    defaultValues: { value: "" },
    onSubmit: ({ value }) => applyMutation.mutate(value.value),
  });
  const applied =
    applyMutation.data?.status === "applied" ||
    applyMutation.data?.status === "already_applied";
  const errorMessage = applied
    ? undefined
    : applyMutation.data?.status === "claimed"
      ? ycPerkApplyErrorMessages.claimed
      : applyMutation.data?.status === "invalid"
        ? ycPerkApplyErrorMessages[applyMutation.data.reason]
        : applyMutation.data?.status === "invalid_code"
          ? ycPerkApplyErrorMessages.invalid
          : applyMutation.data?.status === "invalid_input"
            ? applyMutation.data.message
            : applyMutation.isError
              ? "We couldn’t apply this. Try again."
              : perk === "claimed"
                ? ycPerkApplyErrorMessages.claimed
                : perk === "invalid"
                  ? ycPerkApplyErrorMessages.invalid
                  : undefined;

  if (applied) {
    return (
      <div className="border-t border-[#ede7dc] px-6 py-5 sm:px-8">
        <p className="text-sm leading-6 text-[#756b5d]">
          YC founder year is applied to personal Pro.
        </p>
      </div>
    );
  }

  return (
    <div className="border-t border-[#ede7dc] px-6 py-5 sm:px-8">
      <p className="text-sm leading-6 text-[#756b5d]">
        YC founder? Paste your verification link or Pro code.
      </p>
      <form
        className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-center"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field
          name="value"
          validators={{
            onChange: ({ value }) => validateYcPerkApplyValue(value),
            onBlur: ({ value }) => validateYcPerkApplyValue(value),
            onSubmit: ({ value }) => validateYcPerkApplyValue(value),
          }}
        >
          {(field) => (
            <div className="min-w-0 flex-1">
              <label htmlFor={field.name} className="sr-only">
                YC verification link or promotion code
              </label>
              <input
                id={field.name}
                name={field.name}
                type="text"
                autoComplete="off"
                placeholder="YC verification link or YC- code"
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                className={cn([
                  authInputClassName,
                  "h-9 rounded-full px-4 text-sm",
                  field.state.meta.errors.length > 0
                    ? "border-red-500"
                    : undefined,
                ])}
                aria-invalid={field.state.meta.errors.length > 0}
              />
              <FieldError errors={field.state.meta.errors} />
            </div>
          )}
        </form.Field>
        <button
          type="submit"
          disabled={applyMutation.isPending}
          className={accountPillPrimaryClassName}
        >
          {applyMutation.isPending ? "Applying..." : "Apply perk"}
        </button>
      </form>
      {errorMessage ? (
        <p className="mt-2 text-sm text-red-700" role="alert">
          {errorMessage}
        </p>
      ) : null}
      <p className="mt-2 text-sm text-[#918a80]">
        Need a verification link?{" "}
        <a
          href="https://www.ycombinator.com/verify"
          target="_blank"
          rel="noreferrer"
          className="underline decoration-[#b8afa4] underline-offset-4 transition hover:text-[#181613]"
        >
          Get one from YC
        </a>
        {" · "}
        <Link
          to="/yc/"
          className="underline decoration-[#b8afa4] underline-offset-4 transition hover:text-[#181613]"
        >
          Learn more
        </Link>
      </p>
    </div>
  );
}

function FieldError({ errors }: { errors: Array<unknown> }) {
  const firstError = errors[0];
  const message =
    typeof firstError === "string"
      ? firstError
      : firstError && typeof firstError === "object" && "message" in firstError
        ? String(firstError.message)
        : undefined;

  return message ? (
    <p className="mt-1.5 px-1 text-sm text-red-700" role="alert">
      {message}
    </p>
  ) : null;
}
