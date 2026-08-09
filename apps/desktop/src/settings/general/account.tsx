import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowsClockwise } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import {
  getActionForTier,
  PlanFeatureList,
  PLAN_TIERS,
  type PlanTier,
  type TierAction,
} from "@anlg/pricing";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { SettingsPageTitle } from "~/settings/page-title";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import { buildWebAppUrl } from "~/shared/utils";

export function SettingsAccount() {
  const { t } = useLingui();
  const auth = useAuth();
  const { plan, isPaid, isTrialing, trialDaysRemaining } = useBillingAccess();

  const isAuthenticated = !!auth?.session;
  const [isPending, setIsPending] = useState(false);
  const [isSignOutDialogOpen, setIsSignOutDialogOpen] = useState(false);

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
      await auth?.signOut();
    },
    onSuccess: () => {
      setIsSignOutDialogOpen(false);
      void analyticsCommands.event({
        event: "user_signed_out",
      });
      void analyticsCommands.setProperties({
        set: {
          is_signed_up: false,
        },
      });
    },
    onError: (error) => {
      const message = String(error).includes("unsent local changes")
        ? t`Sync your changes before signing out.`
        : t`Anarlog couldn't sign you out. Try again.`;
      sonnerToast.error(message);
    },
  });

  if (!isAuthenticated) {
    if (isPending) {
      return (
        <div className="flex flex-col gap-8">
          <SettingsPageTitle title={<Trans>Account</Trans>} />
          <Container
            title={<Trans>Finish sign-in</Trans>}
            description={
              <Trans>Finish in your browser, then return to Anarlog.</Trans>
            }
            action={
              <Button onClick={handleSignIn} variant="outline">
                <Trans>Reopen sign-in page</Trans>
              </Button>
            }
          >
            <p className="text-muted-foreground text-xs">
              <Trans>
                If Anarlog stays closed, paste the link in the sign-in window.
              </Trans>
            </p>
          </Container>
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-8">
        <SettingsPageTitle title={<Trans>Account</Trans>} />
        <section className="pb-4">
          <div className="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 flex-1 flex-col gap-4">
              <div className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">
                  <Trans>Sign in to Anarlog</Trans>
                </h3>
                <div className="text-muted-foreground text-sm">
                  <Trans>
                    Sign in for cloud transcription, AI models, and sharing.
                  </Trans>
                </div>
              </div>
              <button
                type="button"
                onClick={handleSignIn}
                className="border-primary bg-primary text-primary-foreground hover:bg-primary/90 h-10 w-fit rounded-full border-2 px-6 text-sm font-medium shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200"
              >
                <Trans>Get started</Trans>
              </button>
            </div>
          </div>
        </section>

        <GuestPlanSection onSignIn={handleSignIn} />
      </div>
    );
  }

  const currentTier = plan === "free" ? "free" : "pro";

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Account</Trans>} />
      <Container
        title={<Trans>Your Account</Trans>}
        description={auth.session?.user.email ?? t`Signed in`}
        action={
          <Button
            variant="destructive"
            onClick={() => setIsSignOutDialogOpen(true)}
            disabled={signOutMutation.isPending}
          >
            {signOutMutation.isPending ? t`Signing out...` : t`Sign out`}
          </Button>
        }
      />

      <DestructiveConfirmationDialog
        open={isSignOutDialogOpen}
        onOpenChange={setIsSignOutDialogOpen}
        title={t`Sign out of Anarlog?`}
        description={t`You'll need to sign in again to use cloud sync and account features.`}
        confirmLabel={t`Sign out`}
        pendingLabel={t`Signing out...`}
        isPending={signOutMutation.isPending}
        onConfirm={() => signOutMutation.mutate()}
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
  const { t } = useLingui();
  const { canStartTrial: canStartTrialQuery, hasPaymentMethod } =
    useBillingAccess();

  const [actionPending, setActionPending] = useState(false);

  // A cardless trial cancels at the end unless a card is added, so replace the
  // static current-plan status with an explicit payment-method action.
  const needsPaymentMethod = isTrialing && !hasPaymentMethod;

  const openBillingUrl = useCallback(
    async (buildUrl: () => Promise<string>) => {
      setActionPending(true);
      try {
        const url = await buildUrl();
        await openUrlWithInstruction(url, "billing", (u) =>
          openerCommands.openUrl(u, null),
        );
      } finally {
        setActionPending(false);
      }
    },
    [],
  );

  const planLabel = currentTier === "free" ? t`Free` : "Pro";
  const trialDaysText =
    trialDaysRemaining == null
      ? null
      : trialDaysRemaining === 1
        ? t`${trialDaysRemaining} day left`
        : t`${trialDaysRemaining} days left`;
  const statusText = isTrialing ? (
    <>
      <Trans>Pro trial</Trans>
      {trialDaysText != null && ` - ${trialDaysText}`}
    </>
  ) : (
    <Trans>
      You're on the <span className="font-semibold">{planLabel}</span> plan
    </Trans>
  );
  const handleOpenBillingPortal = useCallback(() => {
    void openBillingUrl(() => buildWebAppUrl("/app/portal"));
  }, [openBillingUrl]);

  const handleAddPaymentMethod = useCallback(() => {
    void analyticsCommands.event({
      event: "trial_payment_method_clicked",
      days_remaining: trialDaysRemaining,
      source: "settings",
    });

    void openBillingUrl(() =>
      buildWebAppUrl("/app/portal", { intent: "payment_method_update" }),
    );
  }, [openBillingUrl, trialDaysRemaining]);

  const renderAction = (action: TierAction, compact: boolean) => {
    if (action == null) return null;

    if (action.style === "current") {
      if (needsPaymentMethod) {
        if (compact) {
          return (
            <button
              type="button"
              onClick={handleAddPaymentMethod}
              disabled={actionPending}
              className="text-foreground hover:text-foreground text-xs font-medium transition-colors disabled:opacity-50"
            >
              <Trans>Add payment method</Trans>
            </button>
          );
        }

        return (
          <button
            type="button"
            onClick={handleAddPaymentMethod}
            disabled={actionPending}
            className="bg-primary text-primary-foreground hover:bg-primary/90 flex h-8 w-full cursor-pointer items-center justify-center rounded-full text-xs font-medium shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
          >
            <Trans>Add payment method</Trans>
          </button>
        );
      }

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

    const isUpgrade = action.style === "upgrade";

    const handleClick = async () => {
      if (action.label === "Start free trial") {
        void analyticsCommands.event({
          event: "trial_checkout_started",
          plan: "pro",
          period: "monthly",
          source: "settings",
        });

        await openBillingUrl(() =>
          buildWebAppUrl("/app/checkout", {
            period: "monthly",
            trial: "true",
            source: "settings",
          }),
        );
        return;
      }
      const targetPlan = action.targetPlan;
      if (!targetPlan) return;

      void analyticsCommands.event({
        event: "upgrade_clicked",
        plan: targetPlan,
        period: "monthly",
        source: "settings",
      });

      await openBillingUrl(() =>
        buildWebAppUrl("/app/checkout", {
          plan: targetPlan,
          period: "monthly",
          source: "settings",
        }),
      );
    };

    const isBusy = actionPending;
    const label = action.label;

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
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Plan & Billing</Trans>
        </h2>
        {isPaid && (
          <button
            type="button"
            onClick={handleOpenBillingPortal}
            disabled={actionPending}
            className="text-muted-foreground hover:text-muted-foreground text-xs transition-colors disabled:opacity-50"
          >
            <Trans>Manage billing</Trans>
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
  const { t } = useLingui();
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

    const label = action.targetPlan === "pro" ? t`Sign in for Pro` : t`Sign in`;

    if (compact) {
      return (
        <button
          type="button"
          onClick={onSignIn}
          className="text-muted-foreground hover:text-foreground text-xs font-medium transition-colors"
        >
          <Trans>Sign in</Trans>
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
    <section>
      <div className="mb-4 flex flex-col gap-1">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Plans</Trans>
        </h2>
        <p className="text-muted-foreground text-sm">
          <Trans>Compare Free and Pro before you sign in.</Trans>
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
        <div className="grid grid-cols-2">
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div key={tier.id} className="flex flex-col p-3">
                <div className="mb-2 flex items-center gap-2">
                  <span className="text-foreground font-sans text-base font-medium">
                    {tier.name}
                  </span>
                  {isCurrent && isTrialing && (
                    <span className="bg-primary text-primary-foreground rounded-full px-2 py-0.5 text-[10px] font-medium tracking-wide uppercase">
                      <Trans>Trial</Trans>
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
              <div key={tier.id} className="py-3">
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
                        <Trans>Trial</Trans>
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
  const { t } = useLingui();
  const auth = useAuth();
  const handleClick = useCallback(() => {
    void auth.refreshSession();
  }, [auth]);

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={auth.isRefreshingSession}
      className="text-muted-foreground hover:text-muted-foreground transition-colors disabled:opacity-50"
      aria-label={t`Refresh billing status`}
    >
      <ArrowsClockwise
        className={cn(["size-3", auth.isRefreshingSession && "animate-spin"])}
      />
    </button>
  );
}

function Container({
  title,
  description,
  action,
  children,
}: {
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <section>
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
