import { Link } from "@tanstack/react-router";

import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";

import {
  IntegrationPageLayout,
  integrationButtonClassName,
} from "./-integration-ui";
import { getIntegrationDisplay } from "./integration";

function PaywallViewedAnalytics({
  integrationId,
  flow,
}: {
  integrationId: string;
  flow: string;
}) {
  const { track } = useAnalytics();

  useMountEffect(() => {
    track("paywall_viewed", {
      entry_point: "integration_connect",
      feature: "integrations",
      integration: integrationId,
      flow,
    });
  });

  return null;
}

export function UpgradePrompt({
  integrationId,
  flow,
  scheme,
}: {
  integrationId: string;
  flow: string;
  scheme: string;
}) {
  const display = getIntegrationDisplay(integrationId);
  const { analyticsReady } = useAnalytics();

  return (
    <IntegrationPageLayout>
      {analyticsReady && (
        <PaywallViewedAnalytics
          key={`${integrationId}:${flow}`}
          integrationId={integrationId}
          flow={flow}
        />
      )}
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-center gap-2">
          <h1 className="font-sans text-3xl tracking-tight text-stone-700">
            {display.name}
          </h1>
          <span className="rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-700">
            Pro
          </span>
        </div>
        <p className="text-neutral-600">
          Upgrade to Pro to connect {display.name} and other integrations.
        </p>
      </div>

      <div className="flex flex-col gap-3">
        <a href="/#pricing" className={integrationButtonClassName("primary")}>
          Upgrade
        </a>

        {flow === "desktop" ? (
          <button
            onClick={() => {
              window.location.href = `${scheme}://integration/callback?integration_id=${integrationId}&status=upgrade_required`;
            }}
            className="cursor-pointer text-sm text-neutral-500 transition-colors hover:text-neutral-700"
          >
            Back to app
          </button>
        ) : (
          <Link
            to="/app/account/"
            className="text-sm text-neutral-500 transition-colors hover:text-neutral-700"
          >
            Back to account
          </Link>
        )}
      </div>
    </IntegrationPageLayout>
  );
}
