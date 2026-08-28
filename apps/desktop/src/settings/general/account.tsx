import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowsClockwise, PencilSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import {
  colors,
  fonts,
  media,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
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

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { SettingsPageTitle } from "~/settings/page-title";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import { buildWebAppUrl } from "~/shared/utils";

function tierActionLabel(action: NonNullable<TierAction>): string {
  switch (action.kind) {
    case "current":
      return "Current plan";
    case "startTrial":
      return "Start free trial";
    case "checkout":
      return action.direction === "upgrade" ? "Get Pro" : "Switch to Pro";
  }
}

export function SettingsAccount() {
  const { t } = useLingui();
  const auth = useAuth();
  const { plan, isPaid, isTrialing, isPaused, trialDaysRemaining } =
    useBillingAccess();

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
  const openAccountMutation = useMutation({
    mutationFn: async () => {
      const url = await buildWebAppUrl("/app/account");
      await openerCommands.openUrl(url, null);
    },
  });

  if (!isAuthenticated) {
    if (isPending) {
      return (
        <div {...stylex.props(styles.page)}>
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
            <p {...stylex.props(styles.mutedSmall)}>
              <Trans>
                If Anarlog stays closed, paste the link in the sign-in window.
              </Trans>
            </p>
          </Container>
        </div>
      );
    }

    return (
      <div {...stylex.props(styles.page)}>
        <SettingsPageTitle title={<Trans>Account</Trans>} />
        <section {...stylex.props(styles.signInSection)}>
          <div {...stylex.props(styles.signInLayout)}>
            <div {...stylex.props(styles.signInContent)}>
              <div {...stylex.props(styles.signInCopy)}>
                <h3 {...stylex.props(styles.smallHeading)}>
                  <Trans>Sign in to Anarlog</Trans>
                </h3>
                <div {...stylex.props(styles.muted)}>
                  <Trans>
                    Sign in for cloud transcription, AI models, and sharing.
                  </Trans>
                </div>
              </div>
              <button
                type="button"
                onClick={handleSignIn}
                {...stylex.props(styles.getStartedButton)}
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
    <div {...stylex.props(styles.page)}>
      <SettingsPageTitle title={<Trans>Account</Trans>} />
      <Container
        title={<Trans>Your Account</Trans>}
        description={
          auth.session?.user.email ? (
            <button
              type="button"
              onClick={() => openAccountMutation.mutate()}
              disabled={openAccountMutation.isPending}
              {...stylex.props(styles.accountLink)}
            >
              <span>{auth.session.user.email}</span>
              <PencilSimple
                {...stylex.props(styles.tinyIcon)}
                aria-hidden="true"
              />
            </button>
          ) : (
            t`Signed in`
          )
        }
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
        isPaused={isPaused}
        trialDaysRemaining={trialDaysRemaining}
        isPaid={isPaid}
      />
    </div>
  );
}

function PlanBillingSection({
  currentTier,
  isTrialing,
  isPaused,
  trialDaysRemaining,
  isPaid,
}: {
  currentTier: PlanTier;
  isTrialing: boolean;
  isPaused: boolean;
  trialDaysRemaining: number | null;
  isPaid: boolean;
}) {
  const { t } = useLingui();
  const { canStartTrial: canStartTrialQuery, hasPaymentMethod } =
    useBillingAccess();

  const [actionPending, setActionPending] = useState(false);

  // A cardless trial pauses at the end unless a card is added, so replace the
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
  ) : isPaused ? (
    <Trans>Your Pro trial has ended</Trans>
  ) : (
    <Trans>
      You're on the <span {...stylex.props(styles.strong)}>{planLabel}</span>{" "}
      plan
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

    if (action.kind === "current") {
      if (needsPaymentMethod) {
        if (compact) {
          return (
            <button
              type="button"
              onClick={handleAddPaymentMethod}
              disabled={actionPending}
              {...stylex.props(styles.compactCurrentAction)}
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
            {...stylex.props(styles.actionButton, styles.primaryAction)}
          >
            <Trans>Add payment method</Trans>
          </button>
        );
      }

      if (compact) {
        return (
          <span {...stylex.props(styles.mutedSmall)}>
            {tierActionLabel(action)}
          </span>
        );
      }

      return (
        <div {...stylex.props(styles.currentPlan)}>
          {tierActionLabel(action)}
        </div>
      );
    }

    const isUpgrade =
      action.kind === "startTrial" || action.direction === "upgrade";

    const handleClick = async () => {
      if (action.kind === "startTrial") {
        void analyticsCommands.event({
          event: "trial_checkout_started",
          plan: action.plan,
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

      if (isPaused) {
        await openBillingUrl(() => buildWebAppUrl("/app/portal"));
        return;
      }

      void analyticsCommands.event({
        event: "upgrade_clicked",
        plan: action.plan,
        period: "monthly",
        source: "settings",
      });

      await openBillingUrl(() =>
        buildWebAppUrl("/app/checkout", {
          plan: action.plan,
          period: "monthly",
          source: "settings",
        }),
      );
    };

    const isBusy = actionPending;
    const label = isPaused ? t`Resume` : tierActionLabel(action);

    if (compact) {
      return (
        <button
          type="button"
          onClick={handleClick}
          disabled={isBusy}
          {...stylex.props(
            styles.compactAction,
            isUpgrade ? styles.compactUpgrade : styles.compactDowngrade,
          )}
        >
          {label}
        </button>
      );
    }

    return (
      <button
        type="button"
        onClick={handleClick}
        disabled={isBusy}
        {...stylex.props(
          styles.actionButton,
          isUpgrade ? styles.primaryAction : styles.secondaryAction,
        )}
      >
        {label}
      </button>
    );
  };

  return (
    <div>
      <div {...stylex.props(styles.planHeadingRow)}>
        <h2 {...stylex.props(styles.sectionHeading)}>
          <Trans>Plan & Billing</Trans>
        </h2>
        {isPaid && (
          <button
            type="button"
            onClick={handleOpenBillingPortal}
            disabled={actionPending}
            {...stylex.props(styles.manageBilling)}
          >
            <Trans>Manage billing</Trans>
          </button>
        )}
      </div>

      <div {...stylex.props(styles.planStatus)}>
        <p {...stylex.props(styles.muted)}>{statusText}</p>
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

    if (action.kind === "current") {
      if (compact) {
        return (
          <span {...stylex.props(styles.mutedSmall)}>
            {tierActionLabel(action)}
          </span>
        );
      }

      return (
        <div {...stylex.props(styles.currentPlan)}>
          {tierActionLabel(action)}
        </div>
      );
    }

    const label = action.plan === "pro" ? t`Sign in for Pro` : t`Sign in`;

    if (compact) {
      return (
        <button
          type="button"
          onClick={onSignIn}
          {...stylex.props(styles.compactAction, styles.compactUpgrade)}
        >
          <Trans>Sign in</Trans>
        </button>
      );
    }

    return (
      <button
        type="button"
        onClick={onSignIn}
        {...stylex.props(styles.actionButton, styles.primaryAction)}
      >
        {label}
      </button>
    );
  };

  return (
    <section>
      <div {...stylex.props(styles.guestHeading)}>
        <h2 {...stylex.props(styles.sectionHeading)}>
          <Trans>Plans</Trans>
        </h2>
        <p {...stylex.props(styles.muted)}>
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
        <div {...stylex.props(styles.tierGrid)}>
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div key={tier.id} {...stylex.props(styles.tierCard)}>
                <div {...stylex.props(styles.tierNameRow)}>
                  <span {...stylex.props(styles.tierName)}>{tier.name}</span>
                  {isCurrent && isTrialing && (
                    <span {...stylex.props(styles.trialBadge)}>
                      <Trans>Trial</Trans>
                    </span>
                  )}
                </div>

                <div {...stylex.props(styles.tierPriceBlock)}>
                  <span {...stylex.props(styles.tierPrice)}>{tier.price}</span>
                  {tier.period && (
                    <span {...stylex.props(styles.tierPeriod)}>
                      {tier.period}
                    </span>
                  )}
                  {tier.subtitle && (
                    <div {...stylex.props(styles.tierSubtitle)}>
                      {tier.subtitle}
                    </div>
                  )}
                </div>

                <div {...stylex.props(styles.tierFeatures)}>
                  <PlanFeatureList features={tier.features} dense />
                </div>

                <div {...stylex.props(styles.tierAction)}>
                  {renderAction(action, false)}
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div {...stylex.props(styles.tierList)}>
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div key={tier.id} {...stylex.props(styles.tierListItem)}>
                <div {...stylex.props(styles.tierListRow)}>
                  <div {...stylex.props(styles.tierListInfo)}>
                    <span {...stylex.props(styles.tierListName)}>
                      {tier.name}
                    </span>
                    <span {...stylex.props(styles.muted)}>
                      {tier.price}
                      {tier.period}
                    </span>
                    {isCurrent && isTrialing && (
                      <span
                        {...stylex.props(
                          styles.trialBadge,
                          styles.compactTrialBadge,
                        )}
                      >
                        <Trans>Trial</Trans>
                      </span>
                    )}
                  </div>
                  <div {...stylex.props(styles.noShrink)}>
                    {renderAction(action, true)}
                  </div>
                </div>
                <div {...stylex.props(styles.compactFeatures)}>
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
      {...stylex.props(styles.refreshButton)}
      aria-label={t`Refresh billing status`}
    >
      <ArrowsClockwise
        {...stylex.props(
          styles.tinyIcon,
          auth.isRefreshingSession && styles.spinning,
        )}
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
      <div {...stylex.props(styles.containerHeader)}>
        <div {...stylex.props(styles.containerCopy)}>
          <h3 {...stylex.props(styles.smallHeading)}>{title}</h3>
          {description && (
            <div {...stylex.props(styles.muted)}>{description}</div>
          )}
        </div>
        {action ? <div {...stylex.props(styles.noShrink)}>{action}</div> : null}
      </div>
      {children ? (
        <div {...stylex.props(styles.containerChildren)}>{children}</div>
      ) : null}
    </section>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  accountLink: {
    alignItems: "center",
    borderRadius: radii.sm,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 1px ${colors.ring}`,
    },
    color: {
      default: null,
      ":hover": colors.foreground,
    },
    display: "inline-flex",
    gap: "0.375rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
  },
  actionButton: {
    alignItems: "center",
    borderRadius: radii.full,
    cursor: "pointer",
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    transform: {
      default: "scale(1)",
      ":active": "scale(0.98)",
      ":disabled:hover": "scale(1)",
      ":hover": "scale(1.02)",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    width: "100%",
  },
  compactAction: {
    fontSize: "0.75rem",
    fontWeight: 500,
    transitionDuration: "150ms",
    transitionProperty: "color",
  },
  compactCurrentAction: {
    color: colors.foreground,
    fontSize: "0.75rem",
    fontWeight: 500,
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
  },
  compactDowngrade: {
    color: colors.mutedForeground,
  },
  compactFeatures: {
    marginTop: "0.5rem",
  },
  compactTrialBadge: {
    paddingBlock: "1px",
    paddingInline: "0.375rem",
  },
  compactUpgrade: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  containerChildren: {
    marginTop: "1rem",
  },
  containerCopy: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    gap: "0.5rem",
    minWidth: 0,
  },
  containerHeader: {
    alignItems: {
      default: null,
      [media.sm]: "flex-start",
    },
    display: "flex",
    flexDirection: {
      default: "column",
      [media.sm]: "row",
    },
    gap: "1rem",
    justifyContent: {
      default: null,
      [media.sm]: "space-between",
    },
  },
  currentPlan: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    height: "2rem",
    justifyContent: "center",
    width: "100%",
  },
  getStartedButton: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in srgb, ${colors.primary} 90%, transparent)`,
    },
    borderColor: colors.primary,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow: "0 4px 14px rgb(87 83 78 / 0.4)",
    color: colors.primaryForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    height: "2.5rem",
    paddingInline: "1.5rem",
    transitionDuration: "200ms",
    transitionProperty: "all",
    width: "fit-content",
  },
  guestHeading: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
    marginBottom: "1rem",
  },
  manageBilling: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
  },
  muted: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  mutedSmall: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  noShrink: {
    flexShrink: 0,
  },
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
  planHeadingRow: {
    alignItems: "center",
    display: "flex",
    justifyContent: "space-between",
    marginBottom: "0.5rem",
  },
  planStatus: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "1rem",
  },
  primaryAction: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in srgb, ${colors.primary} 90%, transparent)`,
    },
    boxShadow: {
      default: shadows.sm,
      ":hover": shadows.lg,
    },
    color: colors.primaryForeground,
  },
  refreshButton: {
    color: colors.mutedForeground,
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
  },
  secondaryAction: {
    backgroundImage: `linear-gradient(to bottom, ${colors.card}, ${colors.background})`,
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: "0 1px 2px rgb(0 0 0 / 0.05)",
      ":hover": shadows.sm,
    },
    color: colors.mutedForeground,
  },
  sectionHeading: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
  },
  signInContent: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    gap: "1rem",
    minWidth: 0,
  },
  signInCopy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  signInLayout: {
    alignItems: {
      default: null,
      [media.sm]: "center",
    },
    display: "flex",
    flexDirection: {
      default: "column",
      [media.sm]: "row",
    },
    gap: "1.5rem",
    justifyContent: {
      default: null,
      [media.sm]: "space-between",
    },
  },
  signInSection: {
    paddingBottom: "1rem",
  },
  smallHeading: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  strong: {
    fontWeight: 600,
  },
  tierAction: {
    marginTop: "auto",
  },
  tierCard: {
    display: "flex",
    flexDirection: "column",
    padding: "0.75rem",
  },
  tierFeatures: {
    marginBottom: "0.75rem",
  },
  tierGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
  },
  tierList: {
    display: "flex",
    flexDirection: "column",
  },
  tierListInfo: {
    alignItems: "center",
    display: "flex",
    flexWrap: "wrap",
    columnGap: "0.5rem",
    minWidth: 0,
    rowGap: "0.25rem",
  },
  tierListItem: {
    paddingBlock: "0.75rem",
  },
  tierListName: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  tierListRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
  },
  tierName: {
    color: colors.foreground,
    fontFamily: fonts.sans,
    fontSize: "1rem",
    fontWeight: 500,
  },
  tierNameRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.5rem",
  },
  tierPeriod: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    marginLeft: "0.25rem",
  },
  tierPrice: {
    color: colors.mutedForeground,
    fontFamily: fonts.sans,
    fontSize: "1.25rem",
  },
  tierPriceBlock: {
    marginBottom: "0.5rem",
  },
  tierSubtitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.125rem",
  },
  tinyIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  trialBadge: {
    backgroundColor: colors.primary,
    borderRadius: radii.full,
    color: colors.primaryForeground,
    fontSize: "0.625rem",
    fontWeight: 500,
    letterSpacing: "0.025em",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
    textTransform: "uppercase",
  },
});
