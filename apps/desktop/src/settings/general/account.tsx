import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { type ReactNode, useCallback, useState } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { PencilSimple } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { AccountProfile } from "./account-profile";

import { useAuth } from "~/auth";
import { SettingsPageTitle } from "~/settings/page-title";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import { buildWebAppUrl } from "~/shared/utils";

export function SettingsAccount() {
  const { t } = useLingui();
  const auth = useAuth();
  const isAuthenticated = !!auth?.session;
  const [isPending, setIsPending] = useState(false);
  const [isSignOutDialogOpen, setIsSignOutDialogOpen] = useState(false);

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
      setIsPending(false);
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
  const openConnectedAccounts = useMutation({
    mutationFn: async () => {
      if (!auth?.session) return;
      const url = new URL(
        await buildWebAppUrl("/app/account", {
          flow: "web",
          section: "connected-accounts",
          account_user_id: auth.session.user.id,
        }),
      );
      url.hash = "connected-accounts";
      await openerCommands.openUrl(url.toString(), null);
    },
    onError: () =>
      sonnerToast.error(t`Couldn't open connected accounts. Try again.`),
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
        <section className="flex min-w-0 flex-col items-start gap-4 pb-4">
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
            className="border-primary bg-primary text-primary-foreground hover:bg-primary/90 rounded-pill h-10 border-2 px-6 text-sm font-medium shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200 [corner-shape:round]"
          >
            <Trans>Get started</Trans>
          </button>
        </section>

        <AccountProfile />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Account</Trans>} />
      <Container
        title={<Trans>Your Account</Trans>}
        description={
          auth.session?.user.email ? (
            <button
              type="button"
              onClick={() => openAccountMutation.mutate()}
              disabled={openAccountMutation.isPending}
              className="hover:text-foreground focus-visible:ring-ring inline-flex items-center gap-1.5 rounded-sm transition-colors focus-visible:ring-1 focus-visible:outline-hidden disabled:opacity-50"
            >
              <span>{auth.session.user.email}</span>
              <PencilSimple className="size-3" aria-hidden="true" />
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

      <Container
        title={<Trans>Connected accounts</Trans>}
        description={
          <Trans>Manage your sign-in methods on the Anarlog website.</Trans>
        }
        action={
          <Button
            variant="outline"
            disabled={openConnectedAccounts.isPending}
            onClick={() => openConnectedAccounts.mutate()}
          >
            <Trans>Manage sign-in methods</Trans>
          </Button>
        }
      />

      <AccountProfile />

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
    </div>
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
      <div className="flex min-w-0 flex-col gap-4 @sm:flex-row @sm:items-start @sm:justify-between">
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
