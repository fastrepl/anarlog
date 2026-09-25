import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import {
  DEFAULT_DESKTOP_SCHEME,
  flowSearchSchema,
} from "@/functions/desktop-flow";
import { useBilling } from "@/hooks/use-billing";
import { getIntegrationBillingGate } from "@/lib/integration-billing-gate";
import { useNangoSessionHandoffToken } from "@/lib/integration-handoff";

import { IntegrationButton, IntegrationPageLayout } from "./-integration-ui";
import { ConnectFlow } from "./-integrations-connect-flow";
import { DisconnectFlow } from "./-integrations-disconnect-flow";
import { UpgradePrompt } from "./-integrations-upgrade-prompt";

const commonSearch = {
  integration_id: z.string().default("google-calendar"),
  connection_id: z.string().optional(),
  action: z.enum(["connect", "reconnect", "disconnect"]).default("connect"),
  return_to: z.string().optional(),
  handoff: z.literal("nango").optional(),
};

const validateSearch = flowSearchSchema(commonSearch);

const INTEGRATION_DISPLAY: Record<
  string,
  { name: string; description: string; connectingHint: string }
> = {
  "google-calendar": {
    name: "Google Calendar",
    description: "See upcoming events and link them to private notes",
    connectingHint: "Finish authorization with Google, then return to Anarlog",
  },
  outlook: {
    name: "Outlook Calendar",
    description: "See upcoming events and link them to private notes",
    connectingHint:
      "Finish authorization with Microsoft, then return to Anarlog",
  },
  linear: {
    name: "Linear",
    description: "Sync your issues and tasks",
    connectingHint: "Follow the prompts to connect your Linear account",
  },
  github: {
    name: "GitHub",
    description: "Sync your issues and pull requests",
    connectingHint: "Follow the prompts to connect your GitHub account",
  },
  slack: {
    name: "Slack",
    description: "Send meeting recaps to your channels",
    connectingHint: "Finish authorization with Slack, then return to Anarlog",
  },
  notion: {
    name: "Notion",
    description: "Add meeting updates to your pages",
    connectingHint: "Pick the Notion pages to share, then return to Anarlog",
  },
  zoom: {
    name: "Zoom",
    description: "Sync your Zoom cloud recordings",
    connectingHint: "Finish authorization with Zoom, then return to Anarlog",
  },
  fathom: {
    name: "Fathom",
    description: "Sync your Fathom meeting recordings",
    connectingHint: "Finish authorization with Fathom, then return to Anarlog",
  },
  webex: {
    name: "Webex",
    description: "Sync your Webex meeting transcripts",
    connectingHint: "Finish authorization with Webex, then return to Anarlog",
  },
  "google-meet": {
    name: "Google Meet",
    description: "Sync your Google Meet transcripts",
    connectingHint: "Finish authorization with Google, then return to Anarlog",
  },
  "microsoft-teams": {
    name: "Microsoft Teams",
    description: "Sync your Teams meeting transcripts",
    connectingHint:
      "Finish authorization with Microsoft, then return to Anarlog",
  },
  attio: {
    name: "Attio",
    description: "Connect Attio to look up your contacts",
    connectingHint: "Follow the prompts to connect your Attio workspace",
  },
  close: {
    name: "Close",
    description: "Connect Close to look up your contacts",
    connectingHint: "Follow the prompts to connect your Close account",
  },
  hubspot: {
    name: "HubSpot",
    description: "Connect HubSpot to look up your contacts",
    connectingHint: "Follow the prompts to connect your HubSpot account",
  },
  pipedrive: {
    name: "Pipedrive",
    description: "Connect Pipedrive to look up your contacts",
    connectingHint: "Follow the prompts to connect your Pipedrive account",
  },
  salesforce: {
    name: "Salesforce",
    description: "Connect Salesforce to look up your contacts",
    connectingHint: "Follow the prompts to connect your Salesforce account",
  },
};

export function getIntegrationDisplay(integrationId: string) {
  return (
    INTEGRATION_DISPLAY[integrationId] ?? {
      name: integrationId,
      description: `Connect ${integrationId} to sync your data`,
      connectingHint: "Follow the prompts to complete the connection",
    }
  );
}

export const Route = createFileRoute("/_view/app/integration")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

function Component() {
  const search = Route.useSearch();
  const isDesktopHandoff =
    search.flow === "desktop" &&
    search.handoff === "nango" &&
    (search.action === "connect" || search.action === "reconnect");

  if (isDesktopHandoff) {
    return <DesktopHandoffConnect />;
  }

  return <BrowserAuthorizedIntegration />;
}

function DesktopHandoffConnect() {
  const desktopSessionToken = useNangoSessionHandoffToken();

  if (desktopSessionToken === undefined) {
    return (
      <IntegrationPageLayout>
        <p className="text-neutral-500">Loading...</p>
      </IntegrationPageLayout>
    );
  }

  if (!desktopSessionToken) {
    return (
      <IntegrationPageLayout>
        <div className="flex flex-col gap-4">
          <p className="text-neutral-600">
            This connection link is invalid or expired. Return to Anarlog and
            try again.
          </p>
        </div>
      </IntegrationPageLayout>
    );
  }

  return <ConnectFlow sessionToken={desktopSessionToken} />;
}

function BrowserAuthorizedIntegration() {
  const search = Route.useSearch();
  const billing = useBilling();
  const billingVerification = useQuery({
    queryKey: [
      "billing",
      "integration-verification",
      search.flow,
      search.integration_id,
      search.action,
      search.connection_id ?? "",
    ],
    queryFn: billing.refreshBilling,
    enabled: search.action !== "disconnect" && billing.isReady,
    refetchOnMount: "always",
    staleTime: Infinity,
    retry: 1,
  });

  const gate = getIntegrationBillingGate({
    action: search.action,
    isBillingReady: billing.isReady,
    isVerifying:
      billingVerification.isPending || billingVerification.isFetching,
    verificationFailed: billingVerification.isError,
    verifiedIsPaid: billingVerification.data?.isPaid,
  });

  if (gate === "disconnect") {
    return <DisconnectFlow />;
  }

  if (gate === "loading") {
    return (
      <IntegrationPageLayout>
        <p className="text-neutral-500">Loading...</p>
      </IntegrationPageLayout>
    );
  }

  if (gate === "retry") {
    return (
      <IntegrationPageLayout>
        <div className="flex flex-col gap-4">
          <p className="text-neutral-600">
            We couldn’t verify your plan. Please try again.
          </p>
          <IntegrationButton onClick={() => void billingVerification.refetch()}>
            Try again
          </IntegrationButton>
        </div>
      </IntegrationPageLayout>
    );
  }

  if (gate === "upgrade") {
    return (
      <UpgradePrompt
        integrationId={search.integration_id}
        flow={search.flow}
        scheme={search.scheme ?? DEFAULT_DESKTOP_SCHEME}
      />
    );
  }

  return <ConnectFlow />;
}
