import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";

import { radii } from "@anlg/design-system/tokens.stylex";

import { authStyles } from "@/components/auth-shell";
import { getAccountSubscription } from "@/functions/billing";
import { applyYcPerk } from "@/functions/yc-perk";
import { getAccountPlanCopy } from "@/lib/account-plan";
import { validateYcPerkApplyValue } from "@/lib/yc-perk";

import { useAccountSession } from "./-account-session";
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
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style3: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#181613",
  },
  style4: {
    backgroundColor: "#fff0b3",
    paddingInline: ".25rem",
    fontWeight: 600,
  },
  style5: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style6: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#ede7dc",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBlock: "1.25rem",
  },
  style7: {
    marginTop: ".75rem",
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 40rem)": "row",
    },
    gap: ".75rem",
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
  },
  style8: {
    minWidth: 0,
    flexGrow: 1,
  },
  style9: {
    clipPath: "inset(50%)",
    whiteSpace: "nowrap",
    borderWidth: 0,
    width: "1px",
    height: "1px",
    margin: "-1px",
    padding: 0,
    position: "absolute",
    overflow: "hidden",
  },
  style10: {
    marginTop: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style11: {
    marginTop: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#918a80",
  },
  style12: {
    textDecorationLine: "underline",
    textDecorationColor: "#b8afa4",
    textUnderlineOffset: "4px",
    transitionProperty: "color, text-decoration-color",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    color: {
      default: null,
      ":hover": "#181613",
    },
  },
  style13: {
    marginTop: ".375rem",
    paddingInline: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  perkInput: {
    borderRadius: radii.full,
    fontSize: ".875rem",
    height: "2.25rem",
    paddingInline: "1rem",
  },
  invalidPerkInput: {
    borderColor: "#ef4444",
  },
});
export const accountSubscriptionQueryKey = ["account-subscription"];
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
  const subscriptionQuery = useQuery({
    queryKey: accountSubscriptionQueryKey,
    enabled:
      typeof window !== "undefined" &&
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
  });
  const isCheckingPlan =
    isPending ||
    (billing?.isPaid === true &&
      billing.isTrialing !== true &&
      subscriptionQuery.isPending);
  return (
    <div {...stylex.props(accountStyles.card)}>
      <div {...stylex.props(styles.style1)}>
        {isCheckingPlan ? (
          <p {...stylex.props(styles.style2)}>Checking your plan...</p>
        ) : (
          <>
            <div>
              <p {...stylex.props(styles.style3)}>
                You're on{" "}
                <mark {...stylex.props(styles.style4)}>{planLabel}</mark>
              </p>
              <p {...stylex.props(styles.style5)}>{planDetail}</p>
            </div>
            {billing?.isPaid || billing?.isTrialing ? (
              <Link
                to="/app/portal/"
                {...stylex.props([
                  accountStyles.pill,
                  accountStyles.pillSecondary,
                ])}
              >
                Manage billing
              </Link>
            ) : (
              <Link
                to="/app/checkout/"
                search={{
                  period: "monthly",
                  trial: "false",
                  source: "settings",
                }}
                {...stylex.props([
                  accountStyles.pill,
                  accountStyles.pillPrimary,
                ])}
              >
                {billing?.isPaused ? "Resume Pro" : "Upgrade to Pro"}
              </Link>
            )}
          </>
        )}
      </div>
      {!isCheckingPlan && !hasYcPerk ? <YcPerkApplyForm perk={perk} /> : null}
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
    mutationFn: (value: string) =>
      applyYcPerk({
        data: {
          value,
        },
      }),
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
    defaultValues: {
      value: "",
    },
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
      <div {...stylex.props(styles.style6)}>
        <p {...stylex.props(styles.style2)}>YC founder year is applied.</p>
      </div>
    );
  }
  return (
    <div {...stylex.props(styles.style6)}>
      <p {...stylex.props(styles.style2)}>
        YC founder? Paste your verification link or Pro code.
      </p>
      <form
        {...stylex.props(styles.style7)}
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
            <div {...stylex.props(styles.style8)}>
              <label htmlFor={field.name} {...stylex.props(styles.style9)}>
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
                {...stylex.props([
                  authStyles.input,
                  styles.perkInput,
                  field.state.meta.errors.length > 0 && styles.invalidPerkInput,
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
          {...stylex.props([accountStyles.pill, accountStyles.pillPrimary])}
        >
          {applyMutation.isPending ? "Applying..." : "Apply perk"}
        </button>
      </form>
      {errorMessage ? (
        <p {...stylex.props(styles.style10)} role="alert">
          {errorMessage}
        </p>
      ) : null}
      <p {...stylex.props(styles.style11)}>
        Need a verification link?{" "}
        <a
          href="https://www.ycombinator.com/verify"
          target="_blank"
          rel="noreferrer"
          {...stylex.props(styles.style12)}
        >
          Get one from YC
        </a>
        {" · "}
        <Link to="/yc/" {...stylex.props(styles.style12)}>
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
    <p {...stylex.props(styles.style13)} role="alert">
      {message}
    </p>
  ) : null;
}
