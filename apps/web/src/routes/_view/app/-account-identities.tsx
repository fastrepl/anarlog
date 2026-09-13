import { Icon } from "@iconify-icon/react";
import { useMutation, useQuery } from "@tanstack/react-query";

import {
  Check,
  EnvelopeSimple,
  PlugsConnected,
} from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import {
  connectAccountIdentity,
  fetchAccountIdentities,
} from "@/functions/account-identities";
import { signOutFn } from "@/functions/auth";
import {
  identityLinkMessages,
  type IdentityLinkStatus,
} from "@/functions/identity-link";
import { resetPrivateRouteAnalyticsIdentity } from "@/lib/private-route-analytics";

import {
  accountCardClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

const identityButtonClassName = cn([
  accountPillSecondaryClassName,
  "w-32 shrink-0 gap-1.5 whitespace-nowrap",
]);

const providers = [
  { id: "google", label: "Google", icon: "logos:google-icon" },
  { id: "apple", label: "Apple", icon: "simple-icons:apple" },
  { id: "azure", label: "Microsoft", icon: "logos:microsoft-icon" },
  { id: "github", label: "GitHub", icon: "logos:github-icon" },
] as const;

function ProviderIcon({ icon }: { icon: string }) {
  return <Icon icon={icon} width="20" height="20" />;
}

export function AccountIdentitiesSection({
  expectedUserId,
  result,
}: {
  expectedUserId: string;
  result?: IdentityLinkStatus;
}) {
  const identities = useQuery({
    queryKey: ["account-identities", expectedUserId],
    queryFn: () => fetchAccountIdentities({ data: { expectedUserId } }),
    staleTime: 0,
  });
  const connect = useMutation({
    mutationFn: async (provider: (typeof providers)[number]["id"]) => {
      const response = await connectAccountIdentity({
        data: { expectedUserId, provider },
      });
      if (response.error) throw new Error(identityLinkMessages[response.error]);
      if (response.url) window.location.assign(response.url);
    },
  });
  const switchAccount = useMutation({
    mutationFn: async () => {
      const response = await signOutFn();
      if (!response.success)
        throw new Error("We couldn't sign you out. Please try again.");
      resetPrivateRouteAnalyticsIdentity();
      const returnTo = `/app/account?section=connected-accounts&account_user_id=${encodeURIComponent(expectedUserId)}#connected-accounts`;
      window.location.assign(
        `/auth?${new URLSearchParams({ flow: "web", redirect: returnTo })}`,
      );
    },
  });
  const accountError = identities.data?.error;
  const message =
    connect.error?.message ??
    switchAccount.error?.message ??
    (identities.error
      ? "We couldn't load your sign-in methods. Please try again."
      : null) ??
    (accountError ? identityLinkMessages[accountError] : null) ??
    (result ? identityLinkMessages[result] : null);

  return (
    <div className={accountCardClassName}>
      <div className="space-y-3 p-6 sm:p-8">
        <p className="text-muted-foreground text-sm leading-6">
          Connect another sign-in method to use the same notes and subscription.
          {identities.data?.email && (
            <>
              {" "}
              You are signed in as{" "}
              <strong className="text-foreground break-all">
                {identities.data.email}
              </strong>
              .
            </>
          )}
        </p>
        <p className="text-muted-foreground text-sm leading-6">
          If you already have separate Anarlog accounts,{" "}
          <a href="mailto:founders@anarlog.so" className="underline">
            contact support
          </a>{" "}
          for help bringing them together.
        </p>
        {message && (
          <p role="status" className="text-foreground text-sm leading-6">
            {message}
          </p>
        )}
        {(accountError === "account_mismatch" ||
          accountError === "signed_out") && (
          <button
            className={accountPillSecondaryClassName}
            disabled={switchAccount.isPending}
            onClick={() => switchAccount.mutate()}
          >
            {switchAccount.isPending
              ? "Opening sign-in..."
              : "Sign in to your app account"}
          </button>
        )}
        {identities.isError && (
          <button
            className={accountPillSecondaryClassName}
            onClick={() => void identities.refetch()}
          >
            Try again
          </button>
        )}
        {identities.isPending && (
          <p role="status" className="text-muted-foreground text-sm">
            Loading sign-in methods...
          </p>
        )}
      </div>
      {identities.data && !accountError && (
        <ul className="divide-border-subtle border-border-subtle divide-y border-t">
          {identities.data.identities
            .filter(
              (identity) =>
                !providers.some(
                  (provider) => provider.id === identity.provider,
                ),
            )
            .map((identity) => (
              <li
                key={identity.id}
                className="flex items-center justify-between gap-4 px-6 py-4 sm:px-8"
              >
                <div className="flex min-w-0 items-center gap-3 text-sm">
                  <span
                    className="text-muted-foreground shrink-0"
                    aria-hidden="true"
                  >
                    <EnvelopeSimple className="size-5" />
                  </span>
                  <div>
                    <p className="text-sm font-medium">
                      {identity.provider === "email"
                        ? "Email"
                        : identity.provider}
                    </p>
                    <p className="text-muted-foreground text-sm break-all">
                      {identity.email}
                    </p>
                  </div>
                </div>
                <span
                  className={cn([
                    identityButtonClassName,
                    "pointer-events-none cursor-default",
                  ])}
                >
                  <Check className="size-4" />
                  Connected
                </span>
              </li>
            ))}
          {providers.map((provider) => {
            const linked = identities.data.identities.filter(
              (identity) => identity.provider === provider.id,
            );
            return (
              <li
                key={provider.id}
                className="flex items-center justify-between gap-4 px-6 py-4 sm:px-8"
              >
                <div className="flex min-w-0 items-center gap-3 text-sm">
                  <span className="shrink-0" aria-hidden="true">
                    <ProviderIcon icon={provider.icon} />
                  </span>
                  <div className="min-w-0">
                    <p className="text-sm font-medium">{provider.label}</p>
                    {linked.map((identity) => (
                      <p
                        key={identity.id}
                        className="text-muted-foreground text-sm break-all"
                      >
                        {identity.email}
                      </p>
                    ))}
                  </div>
                </div>
                {linked.length ? (
                  <span
                    className={cn([
                      identityButtonClassName,
                      "pointer-events-none cursor-default",
                    ])}
                  >
                    <Check className="size-4" />
                    Connected
                  </span>
                ) : (
                  <button
                    aria-label={`Connect ${provider.label}`}
                    className={identityButtonClassName}
                    disabled={connect.isPending || switchAccount.isPending}
                    onClick={() => connect.mutate(provider.id)}
                  >
                    {connect.isPending && connect.variables === provider.id ? (
                      "Connecting..."
                    ) : (
                      <>
                        <PlugsConnected className="size-4" />
                        Connect
                      </>
                    )}
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
