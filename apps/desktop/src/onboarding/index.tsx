import { useQueryClient } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { Volume2Icon, VolumeXIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as sfxCommands } from "@hypr/plugin-sfx";

import { LoginSection } from "./account";
import { CalendarSection } from "./calendar";
import {
  getInitialStep,
  getNextStep,
  getPrevStep,
  getStepStatus,
} from "./config";
import { FinalSection, finishOnboarding } from "./final";
import { FolderLocationSection } from "./folder-location";
import { PermissionsSection } from "./permissions";
import { OnboardingSection } from "./shared";

import { usePermissions } from "~/shared/hooks/usePermissions";
import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export const TabItemOnboarding: TabItem<
  Extract<Tab, { type: "onboarding" }>
> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={
        <span className="group-hover:animate-wiggle inline-block origin-[70%_80%] text-sm">
          👋
        </span>
      }
      title="Welcome"
      selected={tab.active}
      allowPin={false}
      allowClose={false}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

export function TabContentOnboarding({
  tab: _tab,
}: {
  tab: Extract<Tab, { type: "onboarding" }>;
}) {
  const queryClient = useQueryClient();
  const close = useTabs((state) => state.close);
  const currentTab = useTabs((state) => state.currentTab);
  const [isMuted, setIsMuted] = useState(false);
  const [currentStep, setCurrentStep] = useState(getInitialStep);

  const { micPermissionStatus, systemAudioPermissionStatus } = usePermissions();

  const allPermissionsGranted =
    micPermissionStatus.data === "authorized" &&
    systemAudioPermissionStatus.data === "authorized";

  useEffect(() => {
    if (currentStep === "permissions" && allPermissionsGranted) {
      setCurrentStep("login");
    }
  }, [currentStep, allPermissionsGranted]);

  const goNext = useCallback(() => {
    const next = getNextStep(currentStep);
    if (next) setCurrentStep(next);
  }, [currentStep]);

  const goBack = useCallback(() => {
    const prev = getPrevStep(currentStep);
    if (prev) setCurrentStep(prev);
  }, [currentStep]);

  useEffect(() => {
    void analyticsCommands.event({
      event: "onboarding_step_viewed",
      step: currentStep,
      platform: platform(),
    });
  }, [currentStep]);

  useEffect(() => {
    sfxCommands
      .play("BGM")
      .then(() => sfxCommands.setVolume("BGM", 0.2))
      .catch(console.error);
    return () => {
      sfxCommands.stop("BGM").catch(console.error);
    };
  }, []);

  useEffect(() => {
    sfxCommands.setVolume("BGM", isMuted ? 0 : 0.2).catch(console.error);
  }, [isMuted]);

  const handleFinish = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ["onboarding-needed"] });
    if (currentTab) {
      close(currentTab);
    }
  }, [close, currentTab, queryClient]);

  return (
    <StandardTabWrapper>
      <div className="relative flex h-full flex-col">
        <div className="sticky top-0 z-10 flex items-center justify-between bg-white px-6 pt-4 pb-3">
          <h1 className="font-serif text-2xl font-semibold text-neutral-900">
            Welcome to Char
          </h1>
          <button
            onClick={() => setIsMuted((prev) => !prev)}
            className="rounded-full p-1.5 transition-colors hover:bg-neutral-100"
            aria-label={isMuted ? "Unmute" : "Mute"}
          >
            {isMuted ? (
              <VolumeXIcon size={16} className="text-neutral-600" />
            ) : (
              <Volume2Icon size={16} className="text-neutral-600" />
            )}
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          <div className="flex flex-col gap-3 px-6 pb-16">
            <OnboardingSection
              title="Permissions"
              completedTitle="Permissions granted"
              description="Required for best experience"
              status={getStepStatus("permissions", currentStep)}
              onBack={goBack}
              onNext={goNext}
            >
              <PermissionsSection />
            </OnboardingSection>

            <OnboardingSection
              title="Account"
              description="Sign in to unlock Pro features"
              completedTitle="Signed up"
              status={getStepStatus("login", currentStep)}
              onBack={goBack}
              onNext={goNext}
            >
              <LoginSection onContinue={goNext} />
            </OnboardingSection>

            <OnboardingSection
              title="Calendar"
              description="Select calendars to sync"
              completedTitle="Calendar connected"
              status={getStepStatus("calendar", currentStep)}
              onBack={goBack}
              onNext={goNext}
            >
              <CalendarSection onContinue={goNext} />
            </OnboardingSection>

            <OnboardingSection
              title="Storage"
              description="Where your notes and recordings are stored"
              completedTitle="Storage configured"
              status={getStepStatus("folder-location", currentStep)}
              onBack={goBack}
              onNext={goNext}
            >
              <FolderLocationSection onContinue={goNext} />
            </OnboardingSection>

            <OnboardingSection
              title="Ready to go"
              status={getStepStatus("final", currentStep)}
              onBack={goBack}
              onNext={() => void finishOnboarding(handleFinish)}
            >
              <FinalSection onContinue={handleFinish} />
            </OnboardingSection>
          </div>
        </div>
      </div>
    </StandardTabWrapper>
  );
}
