import { Trans } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { OnboardingButton, onboardingSharedStyles } from "../shared";

import { useAuth } from "~/auth";

export function BeforeLogin({ onContinue: _ }: { onContinue: () => void }) {
  const auth = useAuth();
  const [isOpening, setIsOpening] = useState(false);

  const handleSignIn = () => {
    if (isOpening) return;
    setIsOpening(true);
    void auth.signIn().finally(() => setIsOpening(false));
  };

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.actions)}>
        <OnboardingButton
          onClick={handleSignIn}
          disabled={isOpening}
          sx={[onboardingSharedStyles.compactButton, styles.primaryAction]}
        >
          {isOpening ? (
            <CircleNotch
              {...stylex.props([styles.spinner, onboardingSharedStyles.spin])}
              aria-hidden="true"
            />
          ) : null}
          <Trans>Get started</Trans>
        </OnboardingButton>

        <OnboardingButton
          variant="secondary"
          onClick={handleSignIn}
          disabled={isOpening}
          sx={onboardingSharedStyles.compactButton}
        >
          <Trans>Login</Trans>
        </OnboardingButton>
      </div>
    </div>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexDirection: "row",
    gap: "1rem",
  },
  primaryAction: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    opacity: {
      default: 1,
      ":disabled": 0.7,
    },
  },
  root: {
    alignItems: "flex-start",
    display: "flex",
    flexDirection: "column",
  },
  spinner: {
    height: "0.875rem",
    width: "0.875rem",
  },
});
