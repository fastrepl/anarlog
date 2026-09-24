import type { MessageDescriptor } from "@lingui/core";
import { msg } from "@lingui/core/macro";
import { Trans, useLingui } from "@lingui/react/macro";
import { useQueries, useQuery } from "@tanstack/react-query";
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
  PRO_TRIAL_DAYS,
  type TierAction,
} from "@anlg/pricing";
import { ArrowsClockwise } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { requestSyncDevices } from "~/auth/sync-devices";
import { SettingsPageTitle } from "~/settings/page-title";
import { getWorkspaceAccess, requireTeamContext } from "~/settings/team/client";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { buildWebAppUrl } from "~/shared/utils";
import { useTabs } from "~/store/zustand/tabs";

export function SettingsBilling() {
  const auth = useAuth();
  const openNew = useTabs((state) => state.openNew);
  const billing = useBillingAccess();
  const { plan, isPaid, isTrialing, isPaused, trialDaysRemaining } = billing;
  const [billingPeriod, setBillingPeriod] = useState<BillingPeriod>("monthly");

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

  const billingActions = useBillingActions(billingPeriod, trialDaysRemaining);

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Billing</Trans>} />
      {auth.session ? (
        <>
          <PlanBillingSection
            currentTier={currentTier}
            isTrialing={isTrialing}
            isPaused={isPaused}
            trialDaysRemaining={trialDaysRemaining}
            isPaid={isPaid}
            isCurrentTierPending={isCurrentTierPending}
            billingActions={billingActions}
          />
          <PlanLimitsSection
            billing={billing}
            workspaces={workspaces.data ?? []}
            workspaceAccess={workspaceAccess}
          />
          <PlansSection
            currentTier={currentTier}
            isTrialing={isTrialing}
            isPaused={isPaused}
            isCurrentTierPending={isCurrentTierPending}
            billingPeriod={billingPeriod}
            onBillingPeriodChange={setBillingPeriod}
            billingActions={billingActions}
          />
        </>
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

const BILLING_DATE_FORMAT: Intl.DateTimeFormatOptions = {
  year: "numeric",
  month: "short",
  day: "numeric",
};

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

function useBillingActions(
  billingPeriod: BillingPeriod,
  trialDaysRemaining: number | null,
) {
  const [actionPending, setActionPending] = useState(false);

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

  const openBillingPortal = useCallback(() => {
    void openBillingUrl(() => buildWebAppUrl("/app/portal"));
  }, [openBillingUrl]);

  const addPaymentMethod = useCallback(() => {
    void analyticsCommands.event({
      event: "trial_payment_method_clicked",
      days_remaining: trialDaysRemaining,
      source: "settings",
    });

    void openBillingUrl(() =>
      buildWebAppUrl("/app/portal", { intent: "payment_method_update" }),
    );
  }, [openBillingUrl, trialDaysRemaining]);

  const openEnterprise = useCallback(async () => {
    setActionPending(true);
    try {
      await openerCommands.openUrl("https://anarlog.so/enterprise/", null);
    } finally {
      setActionPending(false);
    }
  }, []);

  const runTierAction = useCallback(
    async (action: TierAction, isPaused: boolean) => {
      if (!action || action.kind === "current") {
        return;
      }

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
    },
    [billingPeriod, openBillingUrl],
  );

  return {
    actionPending,
    openBillingPortal,
    addPaymentMethod,
    openEnterprise,
    runTierAction,
  };
}

const pillChipClassName =
  "rounded-pill px-2 py-0.5 text-[10px] font-medium transition-colors [corner-shape:round] disabled:opacity-50";
const pillButtonClassName =
  "rounded-pill px-3 py-1.5 text-xs font-medium transition-colors [corner-shape:round] disabled:opacity-50";

function PlanBillingSection({
  currentTier,
  isTrialing,
  isPaused,
  trialDaysRemaining,
  isPaid,
  isCurrentTierPending,
  billingActions,
}: {
  currentTier: MarketingPlanTier;
  isTrialing: boolean;
  isPaused: boolean;
  trialDaysRemaining: number | null;
  isPaid: boolean;
  isCurrentTierPending: boolean;
  billingActions: ReturnType<typeof useBillingActions>;
}) {
  const { t, i18n } = useLingui();
  const billing = useBillingAccess();
  const openNew = useTabs((state) => state.openNew);
  const { actionPending, openBillingPortal, addPaymentMethod, runTierAction } =
    billingActions;

  const proPrice = getFixedPlanPrice("pro");
  const tierData = PLAN_TIERS.find((tier) => tier.id === currentTier);
  const planLabel =
    currentTier === "free" ? t`Free` : (tierData?.name ?? "Pro");

  // A cardless trial pauses at the end unless a card is added, so replace the
  // static current-plan status with an explicit payment-method action.
  const needsPaymentMethod =
    currentTier === "pro" && isTrialing && !billing.hasPaymentMethod;

  const formattedTrialEnd =
    billing.trialEnd != null
      ? i18n.date(billing.trialEnd, BILLING_DATE_FORMAT)
      : null;
  const formattedPeriodEnd =
    billing.currentPeriodEnd != null
      ? i18n.date(billing.currentPeriodEnd, BILLING_DATE_FORMAT)
      : null;

  const trialDaysText =
    trialDaysRemaining == null
      ? null
      : trialDaysRemaining === 1
        ? t`${trialDaysRemaining} day left`
        : t`${trialDaysRemaining} days left`;

  const statusText = isCurrentTierPending ? null : currentTier === "pro" &&
    isTrialing ? (
    <>
      <Trans>Pro trial</Trans>
      {trialDaysText != null && ` - ${trialDaysText}`}
      {formattedTrialEnd != null && ` · ${t`ends ${formattedTrialEnd}`}`}
    </>
  ) : currentTier !== "team" && currentTier !== "enterprise" && isPaused ? (
    <Trans>Your Pro trial has ended</Trans>
  ) : (
    <>
      <Trans>
        You're on the <span className="font-semibold">{planLabel}</span> plan
      </Trans>
      {isPaid && formattedPeriodEnd != null && (
        <>
          {" · "}
          {billing.cancelAtPeriodEnd
            ? t`ends ${formattedPeriodEnd}`
            : t`renews ${formattedPeriodEnd}`}
        </>
      )}
    </>
  );

  // Paid Pro is intentionally price-less: the claims do not expose whether the
  // subscription is monthly or yearly, so the renewal date is shown instead.
  const priceText = isCurrentTierPending
    ? null
    : currentTier === "pro" && isTrialing && proPrice != null
      ? t`$${proPrice.monthly} /month after trial`
      : currentTier === "pro" && isPaid
        ? null
        : tierData != null
          ? `${tierData.price}${tierData.period}`
          : null;

  const freePlanAction =
    currentTier === "free" && !isPaused
      ? getActionForTier("pro", "free", billing.canStartTrial.data)
      : null;

  const planAction = isCurrentTierPending ? (
    <span
      className="bg-muted rounded-pill inline-block h-7 w-24 animate-pulse [corner-shape:round]"
      aria-hidden="true"
    />
  ) : needsPaymentMethod ? (
    <button
      type="button"
      onClick={addPaymentMethod}
      disabled={actionPending}
      className={cn([
        pillButtonClassName,
        "bg-primary text-primary-foreground hover:bg-primary/90",
      ])}
    >
      <Trans>Add payment method</Trans>
    </button>
  ) : isPaused && currentTier !== "team" && currentTier !== "enterprise" ? (
    <button
      type="button"
      onClick={openBillingPortal}
      disabled={actionPending}
      className={cn([
        pillButtonClassName,
        "bg-primary text-primary-foreground hover:bg-primary/90",
      ])}
    >
      <Trans>Resume</Trans>
    </button>
  ) : currentTier === "team" || currentTier === "enterprise" ? (
    <button
      type="button"
      onClick={() => openNew({ type: "settings", state: { tab: "team" } })}
      className={cn([
        pillButtonClassName,
        "bg-muted text-muted-foreground hover:text-foreground",
      ])}
    >
      <Trans>Open Teams</Trans>
    </button>
  ) : isPaid && currentTier === "pro" ? (
    <button
      type="button"
      onClick={openBillingPortal}
      disabled={actionPending}
      className={cn([
        pillButtonClassName,
        "bg-muted text-muted-foreground hover:text-foreground",
      ])}
    >
      <Trans>Manage billing</Trans>
    </button>
  ) : freePlanAction ? (
    <button
      type="button"
      onClick={() => void runTierAction(freePlanAction, false)}
      disabled={actionPending}
      className={cn([
        pillButtonClassName,
        "bg-primary text-primary-foreground hover:bg-primary/90",
      ])}
    >
      {t(tierActionLabel(freePlanAction))}
    </button>
  ) : null;

  return (
    <section>
      <div className="mb-2 flex flex-col gap-1">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Your plan</Trans>
        </h2>
        <p className="text-muted-foreground text-sm">
          <Trans>
            Manage or cancel your subscription in the billing portal.
          </Trans>
        </p>
      </div>

      <div className="border-border/60 flex items-center justify-between gap-4 rounded-xl border px-4 py-4">
        <div className="min-w-0">
          {isCurrentTierPending ? (
            <span
              className="bg-muted block h-5 w-40 animate-pulse rounded"
              aria-hidden="true"
            />
          ) : (
            <>
              <p className="font-sans text-base font-medium">
                {`Anarlog ${planLabel}`}
              </p>
              <div className="text-muted-foreground mt-1 flex flex-col gap-0.5 text-sm">
                {priceText != null && <p>{priceText}</p>}
                {statusText != null && <p>{statusText}</p>}
              </div>
            </>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {planAction}
          <RefreshBillingButton />
        </div>
      </div>
    </section>
  );
}

function UsageLimitRow({
  label,
  sublabel,
  metric,
  fraction,
}: {
  label: string;
  sublabel: ReactNode;
  metric: string;
  fraction: number | null;
}) {
  const percent =
    fraction == null
      ? null
      : Math.min(100, Math.max(0, Math.round(fraction * 100)));

  return (
    <div className="px-4 py-3">
      <div className="flex items-baseline justify-between gap-4">
        <p className="text-sm font-medium">{label}</p>
        <p className="shrink-0 text-sm tabular-nums">{metric}</p>
      </div>
      {sublabel != null && (
        <div className="text-muted-foreground mt-0.5 text-xs">{sublabel}</div>
      )}
      {percent != null && (
        <div
          className="bg-muted mt-2 h-1.5 w-full overflow-hidden rounded-full"
          role="progressbar"
          aria-valuenow={percent}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={label}
        >
          <div
            className="bg-primary h-full rounded-full"
            style={{ width: `${percent}%` }}
          />
        </div>
      )}
    </div>
  );
}

function PlanLimitsSection({
  billing,
  workspaces,
  workspaceAccess,
}: {
  billing: ReturnType<typeof useBillingAccess>;
  workspaces: Array<{ workspaceId: string; name?: string }>;
  workspaceAccess: Array<{
    data?: {
      tier: string;
      seatLimit?: number | null;
      usedSeats?: number;
    };
  }>;
}) {
  const { t, i18n } = useLingui();
  const auth = useAuth();
  const session = auth.session;

  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- Cache by account; rotating access tokens must not become cache keys.
  const devicesQuery = useQuery({
    queryKey: ["sync-devices", session?.user.id],
    queryFn: ({ signal }) => requestSyncDevices(session!.access_token, signal),
    enabled: Boolean(session && billing.isPro),
    staleTime: 60_000,
  });

  const rows: ReactNode[] = [];

  if (billing.isTrialing && billing.trialDaysRemaining != null) {
    const remaining = Math.min(
      Math.max(billing.trialDaysRemaining, 0),
      PRO_TRIAL_DAYS,
    );
    rows.push(
      <UsageLimitRow
        key="trial"
        label={t`Pro trial`}
        sublabel={
          billing.trialEnd != null ? (
            <Trans>
              Ends {i18n.date(billing.trialEnd, BILLING_DATE_FORMAT)}
            </Trans>
          ) : null
        }
        metric={remaining === 1 ? t`1 day left` : t`${remaining} days left`}
        fraction={remaining / PRO_TRIAL_DAYS}
      />,
    );
  }

  const devices = devicesQuery.data;
  if (devices) {
    const used = new Set([
      ...devices.devices.map((device) => device.deviceFingerprint),
      ...devices.pendingDevices.map((device) => device.deviceFingerprint),
    ]).size;
    rows.push(
      <UsageLimitRow
        key="devices"
        label={t`Synced devices`}
        sublabel={used >= devices.maxDevices ? t`At your device limit` : null}
        metric={t`${used} of ${devices.maxDevices} used`}
        fraction={used / devices.maxDevices}
      />,
    );
  }

  workspaces.forEach((workspace, index) => {
    const access = workspaceAccess[index]?.data;
    if (
      !access ||
      (access.tier !== "team" && access.tier !== "enterprise") ||
      typeof access.usedSeats !== "number"
    ) {
      return;
    }
    rows.push(
      <UsageLimitRow
        key={`seats-${workspace.workspaceId}`}
        label={t`Team seats`}
        sublabel={workspace.name ?? null}
        metric={
          access.seatLimit != null
            ? t`${access.usedSeats} of ${access.seatLimit} used`
            : t`${access.usedSeats} in use`
        }
        fraction={
          access.seatLimit != null && access.seatLimit > 0
            ? access.usedSeats / access.seatLimit
            : null
        }
      />,
    );
  });

  if (rows.length === 0) {
    return null;
  }

  return (
    <section>
      <div className="mb-2 flex flex-col gap-1">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Plan limits</Trans>
        </h2>
        <p className="text-muted-foreground text-sm">
          <Trans>Shared across cloud sync and your workspaces.</Trans>
        </p>
      </div>
      <div className="border-border/60 divide-border/60 divide-y rounded-xl border">
        {rows}
      </div>
    </section>
  );
}

function PlansSection({
  currentTier,
  isTrialing,
  isPaused,
  isCurrentTierPending,
  billingPeriod,
  onBillingPeriodChange,
  billingActions,
}: {
  currentTier: MarketingPlanTier;
  isTrialing: boolean;
  isPaused: boolean;
  isCurrentTierPending: boolean;
  billingPeriod: BillingPeriod;
  onBillingPeriodChange: (period: BillingPeriod) => void;
  billingActions: ReturnType<typeof useBillingActions>;
}) {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const openNew = useTabs((state) => state.openNew);
  const { actionPending, addPaymentMethod, openEnterprise, runTierAction } =
    billingActions;

  const proPrice = getFixedPlanPrice("pro");
  const canChooseBillingPeriod =
    !isCurrentTierPending &&
    currentTier === "free" &&
    !isPaused &&
    proPrice?.yearly != null;

  // A cardless trial pauses at the end unless a card is added, so replace the
  // static current-plan status with an explicit payment-method action.
  const needsPaymentMethod =
    currentTier === "pro" && isTrialing && !billing.hasPaymentMethod;

  const renderAction = (tierId: MarketingPlanTier, action: TierAction) => {
    if (tierId === "team") {
      return (
        <button
          type="button"
          onClick={() => openNew({ type: "settings", state: { tab: "team" } })}
          className={cn([
            pillChipClassName,
            "bg-muted text-muted-foreground hover:text-foreground",
          ])}
        >
          <Trans>Open Teams</Trans>
        </button>
      );
    }

    if (tierId === "enterprise") {
      return (
        <button
          type="button"
          onClick={openEnterprise}
          disabled={actionPending}
          className={cn([
            pillChipClassName,
            "bg-muted text-muted-foreground hover:text-foreground",
          ])}
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
          onClick={addPaymentMethod}
          disabled={actionPending}
          className={cn([
            pillChipClassName,
            "bg-primary text-primary-foreground hover:bg-primary/90",
          ])}
        >
          <Trans>Add payment method</Trans>
        </button>
      );
    }

    const isUpgrade =
      action.kind === "startTrial" || action.direction === "upgrade";

    return (
      <button
        type="button"
        onClick={() => void runTierAction(action, isPaused)}
        disabled={actionPending}
        className={cn([
          pillChipClassName,
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
    <section>
      <div className="mb-2 flex min-w-0 items-center justify-between gap-3">
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Compare plans</Trans>
        </h2>
      </div>

      {canChooseBillingPeriod && proPrice?.yearly != null && (
        <BillingPeriodToggle
          value={billingPeriod}
          onChange={onBillingPeriodChange}
          monthlyPrice={proPrice.monthly}
          yearlyPrice={proPrice.yearly}
        />
      )}

      <PlanTierList
        currentTier={isCurrentTierPending ? null : currentTier}
        isTrialing={isTrialing}
        canStartTrial={billing.canStartTrial.data}
        renderAction={renderAction}
      />
    </section>
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
