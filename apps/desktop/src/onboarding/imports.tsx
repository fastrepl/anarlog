import { Trans } from "@lingui/react/macro";

import { OnboardingButton, onboardingSharedStyles } from "./shared";

import { MeetingImportScreen } from "~/imports/screen";

export function ImportSection({
  onContinue,
  onSkip,
}: {
  onContinue: () => void;
  onSkip: () => void;
}) {
  return (
    <MeetingImportScreen
      compact
      onContinue={onContinue}
      onNoSourcesDetected={onSkip}
      secondaryAction={
        <OnboardingButton
          variant="secondary"
          onClick={onSkip}
          sx={onboardingSharedStyles.compactButton}
        >
          <Trans>Skip for now</Trans>
        </OnboardingButton>
      }
    />
  );
}
