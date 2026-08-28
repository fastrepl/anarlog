import * as stylex from "@stylexjs/stylex";
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
const styles = stylex.create({
  style1: {
    color: "#737373",
  },
  style2: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style3: {
    color: "#525252",
  },
});
const commonSearch = {
  integration_id: z.string().default("google-calendar"),
  connection_id: z.string().optional(),
  action: z.enum(["connect", "reconnect", "disconnect"]).default("connect"),
  return_to: z.string().optional(),
  handoff: z.literal("nango").optional(),
};
const validateSearch = flowSearchSchema(commonSearch);
export const INTEGRATION_DISPLAY: Record<
  string,
  {
    name: string;
    description: string;
    connectingHint: string;
  }
> = {
  "google-calendar": {
    name: "Google Calendar",
    description:
      "Review how Anarlog uses Google Calendar data, then continue to Google",
    connectingHint: "Finish authorization with Google, then return to Anarlog",
  },
  outlook: {
    name: "Outlook Calendar",
    description:
      "Review how Anarlog uses Outlook Calendar data, then continue to Microsoft",
    connectingHint:
      "Finish authorization with Microsoft, then return to Anarlog",
  },
  linear: {
    name: "Linear",
    description: "Connect Linear to sync your issues and tasks",
    connectingHint: "Follow the prompts to connect your Linear account",
  },
  github: {
    name: "GitHub",
    description: "Connect GitHub to sync your issues and pull requests",
    connectingHint: "Follow the prompts to connect your GitHub account",
  },
  slack: {
    name: "Slack",
    description: "Connect Slack to send meeting recaps to your channels",
    connectingHint: "Finish authorization with Slack, then return to Anarlog",
  },
  notion: {
    name: "Notion",
    description: "Connect Notion to add meeting updates to your pages",
    connectingHint: "Pick the Notion pages to share, then return to Anarlog",
  },
  zoom: {
    name: "Zoom",
    description:
      "Review how Anarlog uses Zoom cloud recordings, then continue to Zoom",
    connectingHint: "Finish authorization with Zoom, then return to Anarlog",
  },
  fathom: {
    name: "Fathom",
    description:
      "Review how Anarlog uses Fathom meeting recordings, then continue to Fathom",
    connectingHint: "Finish authorization with Fathom, then return to Anarlog",
  },
  webex: {
    name: "Webex",
    description:
      "Review how Anarlog uses Webex meeting transcripts, then continue to Webex",
    connectingHint: "Finish authorization with Webex, then return to Anarlog",
  },
  "google-meet": {
    name: "Google Meet",
    description:
      "Review how Anarlog uses Google Meet transcripts, then continue to Google",
    connectingHint: "Finish authorization with Google, then return to Anarlog",
  },
  "microsoft-teams": {
    name: "Microsoft Teams",
    description:
      "Review how Anarlog uses Teams meeting transcripts, then continue to Microsoft",
    connectingHint:
      "Finish authorization with Microsoft, then return to Anarlog",
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
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
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
        <p {...stylex.props(styles.style1)}>Loading...</p>
      </IntegrationPageLayout>
    );
  }
  if (!desktopSessionToken) {
    return (
      <IntegrationPageLayout>
        <div {...stylex.props(styles.style2)}>
          <p {...stylex.props(styles.style3)}>
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
        <p {...stylex.props(styles.style1)}>Loading...</p>
      </IntegrationPageLayout>
    );
  }
  if (gate === "retry") {
    return (
      <IntegrationPageLayout>
        <div {...stylex.props(styles.style2)}>
          <p {...stylex.props(styles.style3)}>
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
