import { Trans } from "@lingui/react/macro";
import { SpeakerHigh, SpeakerX } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQueryClient } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { commands as sfxCommands } from "@anlg/plugin-sfx";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { LoginSection } from "./account";
import { CalendarSection } from "./calendar";
import {
  getInitialStep,
  getNextStep,
  getPrevStep,
  getStepStatus,
} from "./config";
import { FinalDescription, FinalSection, finishOnboarding } from "./final";
import { FolderLocationSection } from "./folder-location";
import { ImportSection } from "./imports";
import { PermissionsSection } from "./permissions";
import { OnboardingSection } from "./shared";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { StandaloneWindowShell } from "~/shared/window-shell";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function TabContentOnboarding({
  tab: _tab,
}: {
  tab: Extract<Tab, { type: "onboarding" }>;
}) {
  const openCurrent = useTabs((state) => state.openCurrent);

  const handleFinish = useCallback(
    (sessionId: string) => {
      openCurrent({ type: "sessions", id: sessionId });
    },
    [openCurrent],
  );

  return <OnboardingScreen onFinish={handleFinish} />;
}

function OnboardingScreen({
  onFinish,
}: {
  onFinish: (sessionId: string) => void;
}) {
  return <OnboardingScreenContent onFinish={onFinish} headerDragRegion />;
}

export function StandaloneOnboardingScreen({
  onFinish,
}: {
  onFinish: (sessionId: string) => void;
}) {
  return (
    <StandaloneWindowShell>
      <OnboardingScreenContent onFinish={onFinish} headerDragRegion />
    </StandaloneWindowShell>
  );
}

function OnboardingScreenContent({
  onFinish,
  headerDragRegion = false,
}: {
  onFinish: (sessionId: string) => void;
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
    trackAnalyticsEvent("onboarding_step_completed", {
      step: currentStep,
      platform: currentPlatform,
    });
    const next = getNextStep(currentStep);
    if (next) setCurrentStep(next);
  }, [currentPlatform, currentStep]);

  const skipCurrentStep = useCallback(() => {
    trackAnalyticsEvent("onboarding_step_skipped", {
      step: currentStep,
      platform: currentPlatform,
    });
    const next = getNextStep(currentStep);
    if (next) setCurrentStep(next);
  }, [currentPlatform, currentStep]);

  const goBack = useCallback(() => {
    const prev = getPrevStep(currentStep);
    if (prev) setCurrentStep(prev);
  }, [currentStep]);

  const handleCalendarSignIn = useCallback(() => {
    setCurrentStep("login");
    void auth.signIn();
  }, [auth]);

  useEffect(() => {
    trackAnalyticsEvent("onboarding_step_viewed", {
      step: currentStep,
      platform: currentPlatform,
    });
  }, [currentPlatform, currentStep]);

  useEffect(() => {
    sfxCommands.play("BGM").catch(console.error);
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

  const handleFinish = useCallback(
    (sessionId: string) => {
      trackAnalyticsEvent("onboarding_step_completed", {
        step: "final",
        platform: currentPlatform,
      });
      void queryClient.invalidateQueries({ queryKey: ["onboarding-needed"] });
      onFinish(sessionId);
    },
    [currentPlatform, onFinish, queryClient],
  );

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.backdrop)}>
        <motion.div
          {...stylex.props(styles.videoLayer)}
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 2, ease: [0.22, 1, 0.36, 1], delay: 0.4 }}
        >
          <video
            ref={onboardingVideoRef}
            {...stylex.props(styles.video)}
            autoPlay
            loop
            muted
            playsInline
            preload="auto"
            aria-hidden="true"
          >
            <source src="/assets/onboarding-video.mp4" type="video/mp4" />
          </video>
          <div {...stylex.props(styles.videoScrim)} />
        </motion.div>
        <div {...stylex.props(styles.strongBlur)} />
        <div {...stylex.props(styles.softBlur)} />
        <div {...stylex.props(styles.backgroundFade)} />
        <motion.div
          {...stylex.props(styles.introCover)}
          initial={{ opacity: 1 }}
          animate={{ opacity: 0 }}
          transition={{ duration: 1.0, ease: "easeOut", delay: 0.1 }}
        />
      </div>

      <div
        data-tauri-drag-region={headerDragRegion || undefined}
        {...stylex.props(styles.controlsHeader)}
      >
        <button
          onClick={() => setIsMuted((prev) => !prev)}
          data-tauri-drag-region="false"
          {...stylex.props(styles.muteButton)}
          aria-label={isMuted ? "Unmute" : "Mute"}
        >
          {isMuted ? (
            <SpeakerX size={16} {...stylex.props(styles.muteIcon)} />
          ) : (
            <SpeakerHigh size={16} {...stylex.props(styles.muteIcon)} />
          )}
        </button>
      </div>

      <div
        data-tauri-drag-region={headerDragRegion || undefined}
        {...stylex.props(styles.titleHeader)}
      >
        <h1 {...stylex.props(styles.heading)}>
          <Trans>Welcome to Anarlog</Trans>
        </h1>
      </div>

      <div {...mergeStyleXProps(styles.scroll, "scroll-fade-y")}>
        <div {...stylex.props(styles.sections)}>
          <OnboardingSection
            title={<Trans>Start with permissions</Trans>}
            completedTitle={<Trans>Permissions granted</Trans>}
            description={
              currentPlatform === "macos" ? (
                <Trans>
                  Anarlog needs microphone and system audio to transcribe your
                  meetings, plus Accessibility to read meeting controls, visible
                  chat.
                </Trans>
              ) : (
                <Trans>
                  Anarlog needs access to your microphone and system audio to
                  record and transcribe your meetings
                </Trans>
              )
            }
            status={getStepStatus("permissions", currentStep)}
            skippable={false}
            onBack={goBack}
            onNext={goNext}
          >
            <PermissionsSection onContinue={goNext} />
          </OnboardingSection>

          <OnboardingSection
            title={<Trans>Create account</Trans>}
            description={
              <Trans>
                Sign in to unlock powerful AI models, sync across devices, and
                personalization.
              </Trans>
            }
            completedTitle={
              auth.session ? (
                <Trans>Signed in</Trans>
              ) : didSkipLogin ? (
                <Trans>Skipped</Trans>
              ) : (
                <Trans>Account</Trans>
              )
            }
            status={getStepStatus("login", currentStep)}
            onBack={goBack}
            onNext={goNext}
            onSkip={() => {
              setDidSkipLogin(true);
              trackAnalyticsEvent("onboarding_login_skipped");
              trackAnalyticsEvent("onboarding_step_skipped", {
                step: "login",
                platform: currentPlatform,
              });
              const next = getNextStep("login");
              if (next) setCurrentStep(next);
            }}
          >
            <LoginSection
              onContinue={goNext}
              onSkip={() => setDidSkipLogin(true)}
            />
          </OnboardingSection>

          <OnboardingSection
            title={<Trans>Connect calendar</Trans>}
            description={
              <Trans>
                Anarlog will sync your calendar to get meeting reminders
              </Trans>
            }
            completedTitle={<Trans>Calendar connected</Trans>}
            status={getStepStatus("calendar", currentStep)}
            onBack={goBack}
            onNext={goNext}
            onSkip={skipCurrentStep}
          >
            <CalendarSection
              onContinue={goNext}
              onSignIn={handleCalendarSignIn}
            />
          </OnboardingSection>

          <OnboardingSection
            title={<Trans>Bring your meeting history</Trans>}
            description={
              <Trans>
                Import notes and transcripts from the meeting apps you already
                use.
              </Trans>
            }
            completedTitle={<Trans>Meeting history imported</Trans>}
            status={getStepStatus("imports", currentStep)}
            onBack={goBack}
            onNext={goNext}
            onSkip={skipCurrentStep}
          >
            <ImportSection onContinue={goNext} onSkip={skipCurrentStep} />
          </OnboardingSection>

          <OnboardingSection
            title={<Trans>Storage</Trans>}
            description={
              <Trans>Where your notes and recordings are stored</Trans>
            }
            completedTitle={<Trans>Storage configured</Trans>}
            status={getStepStatus("folder-location", currentStep)}
            onBack={goBack}
            onNext={goNext}
            onSkip={skipCurrentStep}
          >
            <FolderLocationSection onContinue={goNext} />
          </OnboardingSection>

          <OnboardingSection
            title={<Trans>Ready to go</Trans>}
            description={<FinalDescription />}
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
  );
}

const styles = stylex.create({
  backdrop: {
    inset: 0,
    overflow: "hidden",
    pointerEvents: "none",
    position: "absolute",
  },
  backgroundFade: {
    backgroundImage: `linear-gradient(to bottom, ${colors.background} 0%, color-mix(in srgb, ${colors.background} 82%, transparent) 18%, color-mix(in srgb, ${colors.background} 97%, transparent) 42%, transparent 100%)`,
    height: "84%",
    left: 0,
    position: "absolute",
    right: 0,
    top: 0,
  },
  controlsHeader: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "3rem",
    justifyContent: "flex-end",
    paddingLeft: "3rem",
    paddingRight: "0.75rem",
    position: "relative",
    zIndex: 30,
  },
  heading: {
    color: colors.foreground,
    fontFamily: fonts.hand,
    fontSize: "2.25rem",
    fontWeight: 600,
    letterSpacing: "normal",
    lineHeight: 1,
  },
  introCover: {
    backgroundColor: colors.background,
    inset: 0,
    position: "absolute",
  },
  muteButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    padding: "0.375rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  muteIcon: {
    color: colors.mutedForeground,
  },
  root: {
    backgroundColor: colors.card,
    display: "flex",
    flexDirection: "column",
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    position: "relative",
  },
  scroll: {
    flex: "1",
    overflowY: "auto",
    position: "relative",
    zIndex: 10,
  },
  sections: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    paddingBottom: "4rem",
    paddingInline: "3rem",
  },
  softBlur: {
    backdropFilter: "blur(12px)",
    height: "92%",
    left: 0,
    maskImage:
      "linear-gradient(to bottom, black, rgb(0 0 0 / 0.8) 34%, rgb(0 0 0 / 0.35) 62%, transparent)",
    position: "absolute",
    right: 0,
    top: 0,
  },
  strongBlur: {
    backdropFilter: "blur(32px)",
    height: "80%",
    left: 0,
    maskImage:
      "linear-gradient(to bottom, black, black 18%, rgb(0 0 0 / 0.9) 36%, rgb(0 0 0 / 0.6) 58%, transparent)",
    position: "absolute",
    right: 0,
    top: 0,
  },
  titleHeader: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    paddingBottom: "2rem",
    paddingInline: "3rem",
    paddingTop: "1rem",
    position: "relative",
    zIndex: 10,
  },
  video: {
    height: "100%",
    inset: 0,
    objectFit: "cover",
    objectPosition: "bottom",
    opacity: 0.28,
    position: "absolute",
    width: "100%",
  },
  videoLayer: {
    inset: 0,
    position: "absolute",
  },
  videoScrim: {
    backgroundImage: `linear-gradient(to top, color-mix(in srgb, ${colors.background} 8%, transparent), color-mix(in srgb, ${colors.background} 18%, transparent), transparent)`,
    inset: 0,
    position: "absolute",
  },
});
