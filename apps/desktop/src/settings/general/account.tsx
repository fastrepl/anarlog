import { useMutation } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { startTrial } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { openUrlWithInstruction } from "@hypr/plugin-windows";
import {
  getActionForTier,
  PlanFeatureList,
  PLAN_TIERS,
  type PlanTier,
  type TierAction,
} from "@hypr/pricing";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { SettingsPageTitle } from "~/settings/page-title";
import { waitForBillingUpdate } from "~/shared/billing";
import { buildWebAppUrl } from "~/shared/utils";

export function SettingsAccount() {
  const auth = useAuth();
  const { plan, isPaid, isTrialing, trialDaysRemaining } = useBillingAccess();

  const isAuthenticated = !!auth?.session;
  const [isPending, setIsPending] = useState(false);

  useEffect(() => {
    if (isAuthenticated) {
      setIsPending(false);
    }
  }, [isAuthenticated]);

  const handleSignIn = useCallback(async () => {
    setIsPending(true);
    try {
      await auth?.signIn();
    } catch {
      setIsPending(false);
    }
  }, [auth]);

  const signOutMutation = useMutation({
    mutationFn: async () => {
      void analyticsCommands.event({
        event: "user_signed_out",
      });
      void analyticsCommands.setProperties({
        set: {
          is_signed_up: false,
        },
      });

      await auth?.signOut();
    },
  });

  if (!isAuthenticated) {
    if (isPending) {
      return (
        <div className="flex flex-col gap-8">
          <SettingsPageTitle title="Account" />
          <Container
            title="Finish sign-in"
            description="Complete the sign-in flow in your browser, then come back here if Anarlog does not reconnect automatically."
            action={
              <Button onClick={handleSignIn} variant="outline">
                Reopen sign-in page
              </Button>
            }
          >
            <p className="text-muted-foreground text-xs">
              If the browser does not reopen Anarlog, use the paste-link
              fallback in the sign-in instruction window.
            </p>
          </Container>
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title="Account" />
        <section className="pb-4">
          <div className="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 flex-1 flex-col gap-4">
              <div className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">Sign in to Anarlog</h3>
                <div className="text-muted-foreground text-sm">
                  Sign in to unlock cloud transcription and AI models, plus Pro
                  features like sharing.
                </div>
              </div>
              <button
                type="button"
                onClick={handleSignIn}
                className="border-primary bg-primary text-primary-foreground hover:bg-primary/90 h-10 w-fit rounded-full border-2 px-6 text-sm font-medium shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200"
              >
                Get started
              </button>
            </div>
          </div>
        </section>

        <GuestPlanSection onSignIn={handleSignIn} />
      </div>
    );
  }

  const currentTier = plan === "trial" ? "pro" : plan;

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="Account" />
      <Container
        title="Your Account"
        description={auth.session?.user.email ?? "Signed in"}
        action={
          <Button
            variant="outline"
            onClick={() => signOutMutation.mutate()}
            disabled={signOutMutation.isPending}
            className={cn([
              "border-alert-border text-alert-foreground hover:bg-alert hover:text-alert-foreground",
            ])}
          >
            {signOutMutation.isPending ? "Signing out..." : "Sign out"}
          </Button>
        }
      />

      <PlanBillingSection
        currentTier={currentTier}
        isTrialing={isTrialing}
        trialDaysRemaining={trialDaysRemaining}
        isPaid={isPaid}
      />
    </div>
  );
}

function PlanBillingSection({
  currentTier,
  isTrialing,
  trialDaysRemaining,
  isPaid,
}: {
  currentTier: PlanTier;
  isTrialing: boolean;
  trialDaysRemaining: number | null;
  isPaid: boolean;
}) {
  const auth = useAuth();
  const { canStartTrial: canStartTrialQuery } = useBillingAccess();

  const startTrialMutation = useMutation({
    mutationFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        throw new Error("Not authenticated");
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { error } = await startTrial({
        client,
        query: { interval: "monthly" },
      });
      if (error) {
        throw error;
      }
    },
    onSuccess: async () => {
      await waitForBillingUpdate(
        () => auth?.refreshSession() ?? Promise.resolve(),
      );
    },
  });

  const [actionPending, setActionPending] = useState(false);

  const openBillingUrl = useCallback(async (url: string) => {
    setActionPending(true);
    try {
      await openUrlWithInstruction(url, "billing", (u) =>
        openerCommands.openUrl(u, null),
      );
    } finally {
      setActionPending(false);
    }
  }, []);

  const planLabel =
    currentTier === "free" ? "Free" : currentTier === "lite" ? "Lite" : "Pro";
  const statusText = isTrialing ? (
    <>
      Pro trial
      {trialDaysRemaining != null &&
        ` \u2014 ${trialDaysRemaining} day${trialDaysRemaining === 1 ? "" : "s"} left`}
    </>
  ) : (
    <>
      You&rsquo;re on the <span className="font-semibold">{planLabel}</span>{" "}
      plan
    </>
  );
  const handleOpenBillingPortal = useCallback(async () => {
    const url = await buildWebAppUrl("/app/portal");
    void openBillingUrl(url);
  }, [openBillingUrl]);

  const renderAction = (action: TierAction, compact: boolean) => {
    if (action == null) return null;

    if (action.style === "current") {
      if (compact) {
        if (!isPaid) {
          return (
            <span className="text-muted-foreground text-xs">
              {action.label}
            </span>
          );
        }

        return (
          <button
            type="button"
            onClick={handleOpenBillingPortal}
            disabled={actionPending}
            className={cn([
              "group text-muted-foreground hover:text-muted-foreground relative min-w-[88px] text-xs font-medium transition-colors disabled:opacity-50",
            ])}
          >
            <span className="block transition-opacity duration-150 group-hover:opacity-0">
              {action.label}
            </span>
            <span className="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity duration-150 group-hover:opacity-100">
              Cancel
            </span>
          </button>
        );
      }

      if (!isPaid) {
        return (
          <div className="border-border bg-muted text-muted-foreground flex h-8 w-full items-center justify-center rounded-full border text-xs">
            {action.label}
          </div>
        );
      }

      return (
        <button
          type="button"
          onClick={handleOpenBillingPortal}
          disabled={actionPending}
          className={cn([
            "group border-border from-card to-background text-muted-foreground relative flex h-8 w-full items-center justify-center overflow-hidden rounded-full border bg-linear-to-b text-xs font-medium shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100",
          ])}
        >
          <span className="transition-opacity duration-150 group-hover:opacity-0">
            {action.label}
          </span>
          <span className="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity duration-150 group-hover:opacity-100">
            Cancel
          </span>
        </button>
      );
    }

    const isUpgrade = action.style === "upgrade";

    const handleClick = async () => {
      if (action.label === "Start free trial") {
        startTrialMutation.mutate();
        return;
      }
      if (!action.targetPlan) return;

      void analyticsCommands.event({
        event: "upgrade_clicked",
        plan: action.targetPlan,
      });

      if (isPaid && action.targetPlan) {
        const url = await buildWebAppUrl("/app/switch-plan", {
          targetPlan: action.targetPlan,
          targetPeriod: "monthly",
        });
        await openBillingUrl(url);
      } else {
        const url = await buildWebAppUrl("/app/checkout", {
          plan: action.targetPlan,
          period: "monthly",
        });
        await openBillingUrl(url);
      }
    };

    const isBusy = actionPending || startTrialMutation.isPending;

    const label =
      action.label === "Start free trial" && startTrialMutation.isPending
        ? "Loading..."
        : action.label;

    if (compact) {
      return (
        <button
          type="button"
          onClick={handleClick}
          disabled={isBusy}
          className={cn([
            "text-xs font-medium transition-colors",
            isUpgrade
              ? "text-muted-foreground hover:text-foreground"
              : "text-muted-foreground hover:text-muted-foreground",
          ])}
        >
          {label}
        </button>
      );
    }

    const buttonClass = cn([
      "flex h-8 w-full cursor-pointer items-center justify-center rounded-full text-xs font-medium transition-all hover:scale-[102%] active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100",
      isUpgrade
        ? "bg-primary text-primary-foreground hover:bg-primary/90 shadow-md hover:shadow-lg"
        : "border-border from-card to-background text-muted-foreground border bg-linear-to-b shadow-xs hover:shadow-md",
    ]);

    return (
      <button
        type="button"
        onClick={handleClick}
        disabled={isBusy}
        className={buttonClass}
      >
        {label}
      </button>
    );
  };

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h2 className="font-sans text-lg font-semibold">Plan & Billing</h2>
        {isPaid && (
          <button
            type="button"
            onClick={handleOpenBillingPortal}
            className="text-muted-foreground hover:text-muted-foreground text-xs transition-colors"
          >
            Manage billing
          </button>
        )}
      </div>

      <div className="mb-4 flex items-center gap-2">
        <p className="text-muted-foreground text-sm">{statusText}</p>
        <RefreshBillingButton />
      </div>

      <PlanTierList
        currentTier={currentTier}
        isTrialing={isTrialing}
        canStartTrial={canStartTrialQuery.data}
        renderAction={renderAction}
      />
    </div>
  );
}

function GuestPlanSection({ onSignIn }: { onSignIn: () => Promise<void> }) {
  const renderAction = (action: TierAction, compact: boolean) => {
    if (action == null) return null;

    if (action.style === "current") {
      if (compact) {
        return (
          <span className="text-muted-foreground text-xs">{action.label}</span>
        );
      }

      return (
        <div className="border-border bg-muted text-muted-foreground flex h-8 w-full items-center justify-center rounded-full border text-xs">
          {action.label}
        </div>
      );
    }

    const label =
      action.targetPlan === "lite"
        ? "Sign in for Lite"
        : action.targetPlan === "pro"
          ? "Sign in for Pro"
          : "Sign in";

    if (compact) {
      return (
        <button
          type="button"
          onClick={onSignIn}
          className="text-muted-foreground hover:text-foreground text-xs font-medium transition-colors"
        >
          Sign in
        </button>
      );
    }

    return (
      <button
        type="button"
        onClick={onSignIn}
        className="bg-primary text-primary-foreground hover:bg-primary/90 flex h-8 w-full cursor-pointer items-center justify-center rounded-full text-xs font-medium shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
      >
        {label}
      </button>
    );
  };

  return (
    <section className="border-border border-t pt-6">
      <div className="mb-4 flex flex-col gap-1">
        <h2 className="font-sans text-lg font-semibold">Plans</h2>
        <p className="text-muted-foreground text-sm">
          Compare Free, Lite, and Pro before you sign in.
        </p>
      </div>

      <PlanTierList
        currentTier="free"
        isTrialing={false}
        canStartTrial={false}
        renderAction={renderAction}
      />
    </section>
  );
}

function PlanTierList({
  currentTier,
  isTrialing,
  canStartTrial,
  renderAction,
}: {
  currentTier: PlanTier;
  isTrialing: boolean;
  canStartTrial: boolean;
  renderAction: (action: TierAction, compact: boolean) => ReactNode;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isWide, setIsWide] = useState(true);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver(([entry]) => {
      setIsWide(entry.contentRect.width >= 480);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={containerRef}>
      {isWide ? (
        <div className="divide-border border-border grid grid-cols-3 divide-x border-t">
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div
                key={tier.id}
                className={cn([
                  "flex flex-col p-3",
                  isCurrent && "bg-background/60",
                ])}
              >
                <div className="mb-2 flex items-center gap-2">
                  <span className="text-foreground font-sans text-base font-medium">
                    {tier.name}
                  </span>
                  {isCurrent && isTrialing && (
                    <span className="bg-primary text-primary-foreground rounded-full px-2 py-0.5 text-[10px] font-medium tracking-wide uppercase">
                      Trial
                    </span>
                  )}
                </div>

                <div className="mb-2">
                  <span className="text-muted-foreground font-sans text-xl">
                    {tier.price}
                  </span>
                  {tier.period && (
                    <span className="text-muted-foreground ml-1 text-sm">
                      {tier.period}
                    </span>
                  )}
                  {tier.subtitle && (
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      {tier.subtitle}
                    </div>
                  )}
                </div>

                <div className="mb-3">
                  <PlanFeatureList features={tier.features} dense />
                </div>

                <div className="mt-auto">{renderAction(action, false)}</div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="flex flex-col">
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div
                key={tier.id}
                className={cn([
                  "border-border border-b py-3 last:border-b-0",
                  isCurrent && "bg-background/60 -mx-2 rounded-md px-2",
                ])}
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                    <span className="text-foreground text-sm font-medium">
                      {tier.name}
                    </span>
                    <span className="text-muted-foreground text-sm">
                      {tier.price}
                      {tier.period}
                    </span>
                    {isCurrent && isTrialing && (
                      <span className="bg-primary text-primary-foreground rounded-full px-1.5 py-px text-[10px] font-medium tracking-wide uppercase">
                        Trial
                      </span>
                    )}
                  </div>
                  <div className="shrink-0">{renderAction(action, true)}</div>
                </div>
                <div className="mt-2">
                  <PlanFeatureList features={tier.features} dense />
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function RefreshBillingButton() {
  const auth = useAuth();
  const handleClick = useCallback(() => {
    auth.refreshSession();
  }, [auth]);

  return (
    <button
      type="button"
      onClick={handleClick}
      className="text-muted-foreground hover:text-muted-foreground transition-colors"
      aria-label="Refresh billing status"
    >
      <RefreshCw className="size-3" />
    </button>
  );
}

function Container({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <section className="border-border border-b pb-4 last:border-b-0">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <h3 className="text-sm font-medium">{title}</h3>
          {description && (
            <div className="text-muted-foreground text-sm">{description}</div>
          )}
        </div>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      {children ? <div className="mt-4">{children}</div> : null}
    </section>
  );
}
