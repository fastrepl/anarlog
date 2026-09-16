import type { MessageDescriptor } from "@lingui/core";
import { msg } from "@lingui/core/macro";
import { Trans, useLingui } from "@lingui/react/macro";
import { useQueries } from "@tanstack/react-query";
import { type ReactNode, useCallback, useRef, useState } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import {
  type BillingPeriod,
  getActionForTier,
  getFixedPlanPrice,
  type MarketingPlanTier,
  PlanFeatureList,
  PLAN_TIERS,
  type TierAction,
} from "@anlg/pricing";
import { ArrowsClockwise } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { SettingsPageTitle } from "~/settings/page-title";
import { getWorkspaceAccess, requireTeamContext } from "~/settings/team/client";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { buildWebAppUrl } from "~/shared/utils";
import { useTabs } from "~/store/zustand/tabs";

export function SettingsBilling() {
  const auth = useAuth();
  const openNew = useTabs((state) => state.openNew);
  const { plan, isPaid, isTrialing, isPaused, trialDaysRemaining } =
    useBillingAccess();

  const workspaces = useMyWorkspacesWithMirror();
  const workspaceAccess = useQueries({
    // Auth includes client methods; scope the cache by user identity instead.
    // eslint-disable-next-line @tanstack/query/exhaustive-deps
    queries: (workspaces.data ?? []).map((workspace) => ({
      queryKey: ["team-access", workspace.workspaceId, auth.session?.user.id],
      enabled: !!auth.session,
      queryFn: () =>
        getWorkspaceAccess(requireTeamContext(auth), workspace.workspaceId),
      retry: false,
    })),
  });
  const workspaceTier = workspaceAccess.some(
    (query) => query.data?.tier === "enterprise",
  )
    ? "enterprise"
    : workspaceAccess.some((query) => query.data?.tier === "team")
      ? "team"
      : null;
  const currentTier: MarketingPlanTier =
    workspaceTier ?? (plan === "free" ? "free" : "pro");
  const isCurrentTierPending =
    workspaces.isPending || workspaceAccess.some((query) => query.isPending);

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Billing</Trans>} />
      {auth.session ? (
        <PlanBillingSection
          currentTier={currentTier}
          isTrialing={isTrialing}
          isPaused={isPaused}
          trialDaysRemaining={trialDaysRemaining}
          isPaid={isPaid}
          isCurrentTierPending={isCurrentTierPending}
        />
      ) : (
        <>
          <button
            type="button"
            className="text-primary self-start text-sm font-medium"
            onClick={() =>
              openNew({ type: "settings", state: { tab: "account" } })
            }
          >
            <Trans>Sign in to Anarlog</Trans>
          </button>
          <GuestPlanSection />
        </>
      )}
    </div>
  );
}

function tierActionLabel(action: NonNullable<TierAction>): MessageDescriptor {
  switch (action.kind) {
    case "current":
      return msg`Current plan`;
    case "startTrial":
      return msg`Start free trial`;
    case "checkout":
      return action.direction === "upgrade" ? msg`Get Pro` : msg`Switch to Pro`;
  }
}

function PlanBillingSection({
  currentTier,
  isTrialing,
  isPaused,
  trialDaysRemaining,
  isPaid,
  isCurrentTierPending,
}: {
  currentTier: MarketingPlanTier;
  isTrialing: boolean;
  isPaused: boolean;
  trialDaysRemaining: number | null;
  isPaid: boolean;
  isCurrentTierPending: boolean;
}) {
  const { t } = useLingui();
  const { canStartTrial: canStartTrialQuery, hasPaymentMethod } =
    useBillingAccess();
  const openNew = useTabs((state) => state.openNew);

  const [actionPending, setActionPending] = useState(false);
  const [billingPeriod, setBillingPeriod] = useState<BillingPeriod>("monthly");
  const proPrice = getFixedPlanPrice("pro");
  const canChooseBillingPeriod =
    !isCurrentTierPending &&
    currentTier === "free" &&
    !isPaused &&
    proPrice?.yearly != null;

  // A cardless trial pauses at the end unless a card is added, so replace the
  // static current-plan status with an explicit payment-method action.
  const needsPaymentMethod =
    currentTier === "pro" && isTrialing && !hasPaymentMethod;

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

  const planLabel =
    currentTier === "free"
      ? t`Free`
      : (PLAN_TIERS.find((tier) => tier.id === currentTier)?.name ?? "Pro");
  const trialDaysText =
    trialDaysRemaining == null
      ? null
      : trialDaysRemaining === 1
        ? t`${trialDaysRemaining} day left`
        : t`${trialDaysRemaining} days left`;
  const statusText = isCurrentTierPending ? (
    <span
      className="bg-muted block h-5 w-40 animate-pulse rounded"
      aria-hidden="true"
    />
  ) : currentTier === "pro" && isTrialing ? (
    <>
      <Trans>Pro trial</Trans>
      {trialDaysText != null && ` - ${trialDaysText}`}
    </>
  ) : currentTier !== "team" && currentTier !== "enterprise" && isPaused ? (
    <Trans>Your Pro trial has ended</Trans>
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

  const handleOpenEnterprise = useCallback(async () => {
    setActionPending(true);
    try {
      await openerCommands.openUrl("https://anarlog.so/enterprise/", null);
    } finally {
      setActionPending(false);
    }
  }, []);

  const renderAction = (tierId: MarketingPlanTier, action: TierAction) => {
    if (tierId === "team") {
      return (
        <button
          type="button"
          onClick={() => openNew({ type: "settings", state: { tab: "team" } })}
          className="bg-muted text-muted-foreground hover:text-foreground rounded-pill px-2 py-0.5 text-[10px] font-medium transition-colors [corner-shape:round]"
        >
          <Trans>Open Teams</Trans>
        </button>
      );
    }

    if (tierId === "enterprise") {
      return (
        <button
          type="button"
          onClick={handleOpenEnterprise}
          disabled={actionPending}
          className="bg-muted text-muted-foreground hover:text-foreground rounded-pill px-2 py-0.5 text-[10px] font-medium transition-colors [corner-shape:round] disabled:opacity-50"
        >
          <Trans>Talk to sales</Trans>
        </button>
      );
    }

    if (action == null) return null;

    if (action.kind === "current") {
      if (!needsPaymentMethod) return null;

      return (
        <button
          type="button"
          onClick={handleAddPaymentMethod}
          disabled={actionPending}
          className="bg-primary text-primary-foreground hover:bg-primary/90 rounded-pill px-2 py-0.5 text-[10px] font-medium transition-colors [corner-shape:round] disabled:opacity-50"
        >
          <Trans>Add payment method</Trans>
        </button>
      );
    }

    const isUpgrade =
      action.kind === "startTrial" || action.direction === "upgrade";

    const handleClick = async () => {
      if (action.kind === "startTrial") {
        void analyticsCommands.event({
          event: "trial_checkout_started",
          plan: action.plan,
          period: billingPeriod,
          source: "settings",
        });

        await openBillingUrl(() =>
          buildWebAppUrl("/app/checkout", {
            period: billingPeriod,
            trial: "true",
            source: "settings",
          }),
        );
        return;
      }

      if (isPaused) {
        await openBillingUrl(() => buildWebAppUrl("/app/portal"));
        return;
      }

      void analyticsCommands.event({
        event: "upgrade_clicked",
        plan: action.plan,
        period: billingPeriod,
        source: "settings",
      });

      await openBillingUrl(() =>
        buildWebAppUrl("/app/checkout", {
          plan: action.plan,
          period: billingPeriod,
          source: "settings",
        }),
      );
    };

    return (
      <button
        type="button"
        onClick={handleClick}
        disabled={actionPending}
        className={cn([
          "rounded-pill px-2 py-0.5 text-[10px] font-medium transition-colors [corner-shape:round] disabled:opacity-50",
          isUpgrade
            ? "bg-primary text-primary-foreground hover:bg-primary/90"
            : "bg-muted text-muted-foreground hover:text-foreground",
        ])}
      >
        {isPaused ? t`Resume` : t(tierActionLabel(action))}
      </button>
    );
  };

  return (
    <div>
      <div className="mb-2 flex min-w-0 items-center justify-between gap-3">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Plan & Billing</Trans>
        </h2>
        {!isCurrentTierPending && isPaid && currentTier === "pro" && (
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

      {canChooseBillingPeriod && proPrice?.yearly != null && (
        <BillingPeriodToggle
          value={billingPeriod}
          onChange={setBillingPeriod}
          monthlyPrice={proPrice.monthly}
          yearlyPrice={proPrice.yearly}
        />
      )}

      <PlanTierList
        currentTier={isCurrentTierPending ? null : currentTier}
        isTrialing={isTrialing}
        canStartTrial={canStartTrialQuery.data}
        renderAction={renderAction}
      />
    </div>
  );
}

function BillingPeriodToggle({
  value,
  onChange,
  monthlyPrice,
  yearlyPrice,
}: {
  value: BillingPeriod;
  onChange: (period: BillingPeriod) => void;
  monthlyPrice: number;
  yearlyPrice: number;
}) {
  const { t } = useLingui();
  const yearlySavings = monthlyPrice * 12 - yearlyPrice;
  const options: Array<{ period: BillingPeriod; label: string }> = [
    { period: "monthly", label: t`Monthly · $${monthlyPrice}/mo` },
    {
      period: "yearly",
      label:
        yearlySavings > 0
          ? t`Yearly · $${yearlyPrice}/yr (save $${yearlySavings})`
          : t`Yearly · $${yearlyPrice}/yr`,
    },
  ];

  return (
    <div
      role="radiogroup"
      aria-label={t`Billing period`}
      className="bg-muted rounded-pill mb-4 inline-flex items-center gap-0.5 p-0.5 [corner-shape:round]"
    >
      {options.map((option) => {
        const selected = option.period === value;
        return (
          <button
            key={option.period}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(option.period)}
            className={cn([
              "rounded-pill px-2.5 py-1 text-xs font-medium transition-colors [corner-shape:round]",
              selected
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            ])}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

function GuestPlanSection() {
  return (
    <section>
      <div className="mb-4 flex flex-col gap-1">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Plans</Trans>
        </h2>
        <p className="text-muted-foreground text-sm">
          <Trans>Compare Free, Pro, Team, and Enterprise.</Trans>
        </p>
      </div>

      <PlanTierList
        currentTier="free"
        isTrialing={false}
        canStartTrial={false}
      />
    </section>
  );
}

function PlanStatusChip({
  children,
  emphasis = false,
}: {
  children: ReactNode;
  emphasis?: boolean;
}) {
  return (
    <span
      className={cn([
        "rounded-pill px-2 py-0.5 text-[10px] font-medium [corner-shape:round]",
        emphasis
          ? "bg-primary text-primary-foreground"
          : "bg-muted text-muted-foreground",
      ])}
    >
      {children}
    </span>
  );
}

function PlanTierList({
  currentTier,
  isTrialing,
  canStartTrial,
  renderAction,
}: {
  currentTier: MarketingPlanTier | null;
  isTrialing: boolean;
  canStartTrial: boolean;
  renderAction?: (tierId: MarketingPlanTier, action: TierAction) => ReactNode;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isWide, setIsWide] = useState(true);
  const highlightPro = currentTier === "free";

  useMountEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver(([entry]) => {
      setIsWide(entry.contentRect.width >= 480);
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

  return (
    <div ref={containerRef}>
      <div
        className={cn([
          isWide ? "grid grid-cols-2 gap-x-10 gap-y-8" : "flex flex-col",
        ])}
      >
        {PLAN_TIERS.map((tier) => {
          const isCurrent = tier.id === currentTier;
          const isPro = tier.id === "pro";
          const action =
            (tier.id === "free" || tier.id === "pro") &&
            (currentTier === "free" || currentTier === "pro")
              ? getActionForTier(tier.id, currentTier, canStartTrial)
              : null;
          const chips = (
            <>
              {isCurrent && (
                <PlanStatusChip>
                  <Trans>Current</Trans>
                </PlanStatusChip>
              )}
              {isCurrent && isPro && isTrialing && (
                <PlanStatusChip emphasis>
                  <Trans>Trial</Trans>
                </PlanStatusChip>
              )}
              {renderAction?.(tier.id, action)}
            </>
          );
          const details =
            highlightPro && tier.id === "free" ? (
              <p className="text-muted-foreground text-xs">
                <Trans>
                  On-device transcription, recordings, and your own keys.
                </Trans>
              </p>
            ) : (
              <div className="flex flex-col gap-3">
                <p className="text-muted-foreground text-xs leading-5">
                  {tier.description}
                </p>
                <PlanFeatureList features={tier.features} dense />
              </div>
            );

          if (!isWide) {
            return (
              <div key={tier.id} className="py-3">
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="text-foreground text-sm font-medium">
                    {tier.name}
                  </span>
                  <span className="text-muted-foreground text-sm">
                    {tier.price}
                    {tier.period}
                  </span>
                  {chips}
                </div>
                <div className="mt-2">{details}</div>
              </div>
            );
          }

          return (
            <div key={tier.id} className="flex flex-col">
              <div className="mb-2 flex flex-wrap items-center gap-2">
                <span
                  className={cn([
                    "text-foreground font-sans text-base",
                    isPro && highlightPro ? "font-semibold" : "font-medium",
                  ])}
                >
                  {tier.name}
                </span>
                {chips}
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

              {details}
            </div>
          );
        })}
      </div>
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
