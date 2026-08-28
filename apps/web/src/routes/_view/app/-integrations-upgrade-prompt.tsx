import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";

import { fonts, radii } from "@anlg/design-system/tokens.stylex";

import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";

import {
  IntegrationPageLayout,
  integrationButtonStyles,
} from "./-integration-ui";
import { getIntegrationDisplay } from "./integration";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style2: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: ".5rem",
  },
  style3: {
    fontFamily: fonts.sans,
    fontSize: "1.875rem",
    lineHeight: "2.25rem",
    letterSpacing: "-.025em",
    color: "#44403c",
  },
  style4: {
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#fde68a",
    backgroundColor: "#fffbeb",
    paddingInline: ".5rem",
    paddingBlock: ".125rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    fontWeight: 500,
    color: "#b45309",
  },
  style5: {
    color: "#525252",
  },
  style6: {
    cursor: "pointer",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#737373",
      ":hover": "#404040",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style7: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#737373",
      ":hover": "#404040",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
});
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
      <div {...stylex.props(styles.style1)}>
        <div {...stylex.props(styles.style2)}>
          <h1 {...stylex.props(styles.style3)}>{display.name}</h1>
          <span {...stylex.props(styles.style4)}>Pro</span>
        </div>
        <p {...stylex.props(styles.style5)}>
          Upgrade to Pro to connect {display.name} and other integrations.
        </p>
      </div>

      <div {...stylex.props(styles.style1)}>
        <a
          href="/#pricing"
          {...stylex.props(integrationButtonStyles("primary"))}
        >
          Upgrade
        </a>

        {flow === "desktop" ? (
          <button
            onClick={() => {
              window.location.href = `${scheme}://integration/callback?integration_id=${integrationId}&status=upgrade_required`;
            }}
            {...stylex.props(styles.style6)}
          >
            Back to app
          </button>
        ) : (
          <Link to="/app/account/" {...stylex.props(styles.style7)}>
            Back to account
          </Link>
        )}
      </div>
    </IntegrationPageLayout>
  );
}
