import { useQueryClient } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { Volume2Icon, VolumeXIcon } from "lucide-react";
import { motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as sfxCommands } from "@hypr/plugin-sfx";
import { cn } from "@hypr/utils";

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

import { useAuth } from "~/auth";
import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { StandaloneWindowShell } from "~/shared/window-shell";
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
  const close = useTabs((state) => state.close);
  const currentTab = useTabs((state) => state.currentTab);

  const handleFinish = useCallback(() => {
    if (currentTab) {
      close(currentTab);
    }
  }, [close, currentTab]);

  return <OnboardingScreen onFinish={handleFinish} />;
}

export function OnboardingScreen({ onFinish }: { onFinish: () => void }) {
  return (
    <OnboardingScreenContent
      onFinish={onFinish}
      headerClassName="px-12 pt-12 pb-8"
    />
  );
}

export function StandaloneOnboardingScreen({
  onFinish,
}: {
  onFinish: () => void;
}) {
  const isMacOS = platform() === "macos";

  return (
    <StandaloneWindowShell>
      <OnboardingScreenContent
        onFinish={onFinish}
        headerClassName={
          isMacOS ? "pt-12 pr-12 pb-8 pl-20" : "px-12 pt-12 pb-8"
        }
        headerDragRegion
      />
    </StandaloneWindowShell>
  );
}

function OnboardingScreenContent({
  onFinish,
  headerClassName,
  headerDragRegion = false,
}: {
  onFinish: () => void;
  headerClassName: string;
  headerDragRegion?: boolean;
}) {
  const queryClient = useQueryClient();
  const auth = useAuth();
  const [isMuted, setIsMuted] = useState(false);
  const [currentStep, setCurrentStep] = useState(getInitialStep);
  const [didSkipLogin, setDidSkipLogin] = useState(false);
  const onboardingVideoRef = useRef<HTMLVideoElement>(null);
  const currentPlatform = platform();

  const goNext = useCallback(() => {
    const next = getNextStep(currentStep);
    if (next) setCurrentStep(next);
  }, [currentStep]);

  const goBack = useCallback(() => {
    const prev = getPrevStep(currentStep);
    if (prev) setCurrentStep(prev);
  }, [currentStep]);

  const handleCalendarSignIn = useCallback(() => {
    setCurrentStep("login");
    void auth.signIn();
  }, [auth]);

  useEffect(() => {
    void analyticsCommands.event({
      event: "onboarding_step_viewed",
      step: currentStep,
      platform: currentPlatform,
    });
  }, [currentPlatform, currentStep]);

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

  useEffect(() => {
    if (onboardingVideoRef.current) {
      onboardingVideoRef.current.playbackRate = 0.65;
    }
  }, []);

  const handleFinish = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ["onboarding-needed"] });
    onFinish();
  }, [onFinish, queryClient]);

  return (
    <StandardTabWrapper noBorder>
      <div className="relative flex h-full flex-col">
        <div className="pointer-events-none absolute inset-0 overflow-hidden">
          <motion.div
            className="absolute inset-0"
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 2, ease: [0.22, 1, 0.36, 1], delay: 0.4 }}
          >
            <video
              ref={onboardingVideoRef}
              className="absolute inset-0 h-full w-full object-cover object-bottom opacity-28"
              autoPlay
              loop
              muted
              playsInline
              preload="auto"
              aria-hidden="true"
            >
              <source src="/assets/onboarding-video.mp4" type="video/mp4" />
            </video>
            <div className="absolute inset-0 bg-linear-to-t from-stone-50/8 via-stone-50/18 to-transparent" />
          </motion.div>
          <div className="absolute inset-x-0 top-0 h-[80%] [mask-image:linear-gradient(to_bottom,black,black_18%,rgba(0,0,0,0.9)_36%,rgba(0,0,0,0.6)_58%,transparent)] backdrop-blur-[32px]" />
          <div className="absolute inset-x-0 top-0 h-[92%] [mask-image:linear-gradient(to_bottom,black,rgba(0,0,0,0.8)_34%,rgba(0,0,0,0.35)_62%,transparent)] backdrop-blur-[12px]" />
          <div className="absolute inset-x-0 top-0 h-[84%] bg-linear-to-b from-stone-50 via-stone-50/82 via-stone-50/97 via-18% via-42% to-stone-50/0" />
          <motion.div
            className="absolute inset-0 bg-stone-50"
            initial={{ opacity: 1 }}
            animate={{ opacity: 0 }}
            transition={{ duration: 1.0, ease: "easeOut", delay: 0.1 }}
          />
        </div>

        <div
          data-tauri-drag-region={headerDragRegion || undefined}
          className={cn([
            "sticky top-0 z-10 flex items-center justify-between",
            headerClassName,
          ])}
        >
          <h1 className="font-serif text-3xl font-semibold text-neutral-900">
            Welcome to Char
          </h1>
          <button
            onClick={() => setIsMuted((prev) => !prev)}
            data-tauri-drag-region="false"
            className="rounded-full p-1.5 transition-colors hover:bg-neutral-100"
            aria-label={isMuted ? "Unmute" : "Mute"}
          >
            {isMuted ? (
              <VolumeXIcon size={16} className="text-neutral-400" />
            ) : (
              <Volume2Icon size={16} className="text-neutral-600" />
            )}
          </button>
        </div>

        <div className="relative z-10 flex-1 overflow-y-auto">
          <div className="flex flex-col gap-4 px-12 pb-16">
            <OnboardingSection
              title="Start with permissions"
              completedTitle="Permissions granted"
              description="Char needs access to your microphone and system audio to record and transcribe your meetings"
              status={getStepStatus("permissions", currentStep)}
              skippable={false}
              onBack={goBack}
              onNext={goNext}
            >
              <PermissionsSection onContinue={goNext} />
            </OnboardingSection>

            <OnboardingSection
              title="Create account"
              description="Sign in to unlock powerful Al models, sync across devices, personalization, and workflow integrations."
              completedTitle={
                auth.session
                  ? "Signed in"
                  : didSkipLogin
                    ? "Skipped"
                    : "Account"
              }
              status={getStepStatus("login", currentStep)}
              onBack={goBack}
              onNext={goNext}
              onSkip={() => {
                setDidSkipLogin(true);
                void analyticsCommands.event({
                  event: "onboarding_login_skipped",
                });
              }}
            >
              <LoginSection
                onContinue={goNext}
                onSkip={() => setDidSkipLogin(true)}
              />
            </OnboardingSection>

            <OnboardingSection
              title="Connect calendar"
              description="Char will sync your calendar to get meeting reminders"
              completedTitle="Calendar connected"
              status={getStepStatus("calendar", currentStep)}
              onBack={goBack}
              onNext={goNext}
            >
              <CalendarSection
                onContinue={goNext}
                onSignIn={handleCalendarSignIn}
              />
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
              skippable={false}
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
