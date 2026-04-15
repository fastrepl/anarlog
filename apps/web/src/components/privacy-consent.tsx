import { useRouterState } from "@tanstack/react-router";
import { createContext, useContext, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Checkbox } from "@hypr/ui/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { cn } from "@hypr/utils";

import { useMountEffect } from "@/hooks/useMountEffect";
import type { PrivacyConsentRegion } from "@/lib/privacy-consent";

const STORAGE_KEY = "char_web_tracking_consent_v1";
const COOKIE_POLICY_PATH = "/legal/cookies/";
const PRIVACY_POLICY_PATH = "/legal/privacy/";
const PRIMARY_ACTION_BUTTON_CLASS =
  "rounded-full border-0 surface-dark text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]";
const MUTED_ACTION_BUTTON_CLASS =
  "border-0 bg-transparent text-fg-muted shadow-none hover:bg-transparent hover:text-fg";
const COOKIE_CURSOR =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><text y='24' font-size='24'>🍪</text></svg>\"), auto";

type ConsentState = {
  analytics: boolean;
  source: "user" | "gpc";
  updatedAt: string;
};

function shouldHidePrivacyConsent(pathname: string) {
  return ["/admin", "/auth", "/callback/auth"].some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );
}

function useShouldHidePrivacyConsent() {
  return useRouterState({
    select: (state) => shouldHidePrivacyConsent(state.location.pathname),
  });
}

const PrivacyConsentContext = createContext<{
  analyticsEnabled: boolean;
  analyticsChoice: boolean | null;
  isGpcEnabled: boolean;
  isReady: boolean;
  openPreferences: () => void;
  rejectNonEssential: () => void;
  saveAnalyticsChoice: (analytics: boolean) => void;
} | null>(null);

function readStoredConsent(): ConsentState | null {
  if (typeof window === "undefined") {
    return null;
  }

  const rawValue = window.localStorage.getItem(STORAGE_KEY);
  if (!rawValue) {
    return null;
  }

  try {
    const parsedValue = JSON.parse(rawValue) as Partial<ConsentState>;
    if (
      typeof parsedValue.analytics !== "boolean" ||
      (parsedValue.source !== "user" && parsedValue.source !== "gpc") ||
      typeof parsedValue.updatedAt !== "string"
    ) {
      window.localStorage.removeItem(STORAGE_KEY);
      return null;
    }

    return {
      analytics: parsedValue.analytics,
      source: parsedValue.source,
      updatedAt: parsedValue.updatedAt,
    };
  } catch {
    window.localStorage.removeItem(STORAGE_KEY);
    return null;
  }
}

function writeStoredConsent(value: ConsentState) {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
}

function getGlobalPrivacyControlValue() {
  if (typeof navigator === "undefined") {
    return false;
  }

  return Boolean(
    (navigator as Navigator & { globalPrivacyControl?: boolean })
      .globalPrivacyControl,
  );
}

function usePrivacyConsentContext() {
  const context = useContext(PrivacyConsentContext);
  if (!context) {
    throw new Error("Privacy consent context is not available");
  }
  return context;
}

export function usePrivacyConsent() {
  const context = usePrivacyConsentContext();

  return {
    analyticsEnabled: context.analyticsEnabled,
    analyticsChoice: context.analyticsChoice,
    isGpcEnabled: context.isGpcEnabled,
    isReady: context.isReady,
  };
}

export function CookiePreferencesButton() {
  const { openPreferences } = usePrivacyConsentContext();
  const shouldHideConsentUi = useShouldHidePrivacyConsent();

  if (shouldHideConsentUi) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={openPreferences}
      className={cn([
        "text-color-secondary cursor-pointer p-0 font-sans text-sm transition-colors",
        "hover:text-color hover:underline hover:decoration-dotted",
        "hover:cursor-pointer",
      ])}
      style={{ cursor: COOKIE_CURSOR }}
    >
      Cookie preferences
    </button>
  );
}

export function PrivacyConsentProvider({
  children,
  region,
}: {
  children: React.ReactNode;
  region: PrivacyConsentRegion;
}) {
  const shouldHideConsentUi = useShouldHidePrivacyConsent();
  const [consent, setConsent] = useState<ConsentState | null>(null);
  const [draftAnalytics, setDraftAnalytics] = useState(false);
  const [isDialogOpen, setDialogOpen] = useState(false);
  const [isReady, setIsReady] = useState(false);
  const [isGpcEnabled, setIsGpcEnabled] = useState(false);

  useMountEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const gpcEnabled = getGlobalPrivacyControlValue();
    setIsGpcEnabled(gpcEnabled);

    let nextConsent = readStoredConsent();
    if (!nextConsent && gpcEnabled) {
      nextConsent = {
        analytics: false,
        source: "gpc",
        updatedAt: new Date().toISOString(),
      };
      writeStoredConsent(nextConsent);
    }

    setConsent(nextConsent);
    setDraftAnalytics(Boolean(nextConsent?.analytics));
    setIsReady(true);

    const handleStorage = (event: StorageEvent) => {
      if (event.key !== STORAGE_KEY) {
        return;
      }

      const updatedConsent = readStoredConsent();
      setConsent(updatedConsent);
      setDraftAnalytics(Boolean(updatedConsent?.analytics));
    };

    window.addEventListener("storage", handleStorage);
    return () => window.removeEventListener("storage", handleStorage);
  });

  const analyticsEnabled =
    isReady && consent?.analytics === true && !isGpcEnabled;

  const commitConsent = (
    analytics: boolean,
    source: ConsentState["source"] = "user",
  ) => {
    const nextConsent = {
      analytics,
      source,
      updatedAt: new Date().toISOString(),
    } satisfies ConsentState;

    writeStoredConsent(nextConsent);
    setConsent(nextConsent);
    setDraftAnalytics(analytics);
  };

  const rejectNonEssential = () => {
    const shouldReload = consent?.analytics === true;
    commitConsent(false, isGpcEnabled ? "gpc" : "user");
    setDialogOpen(false);

    if (shouldReload) {
      window.location.reload();
    }
  };

  const saveAnalyticsChoice = (analytics: boolean) => {
    if (!analytics) {
      rejectNonEssential();
      return;
    }

    commitConsent(true);
    setDialogOpen(false);
  };

  const handleDialogOpenChange = (value: boolean) => {
    if (value) {
      setDraftAnalytics(Boolean(consent?.analytics));
    }

    setDialogOpen(value);
  };

  const contextValue = {
    analyticsChoice: consent?.analytics ?? null,
    analyticsEnabled,
    isGpcEnabled,
    isReady,
    openPreferences: () => handleDialogOpenChange(true),
    rejectNonEssential,
    saveAnalyticsChoice,
  };

  return (
    <PrivacyConsentContext.Provider value={contextValue}>
      {children}
      {!shouldHideConsentUi ? (
        <CookieConsentBanner isDialogOpen={isDialogOpen} region={region} />
      ) : null}
      {!shouldHideConsentUi ? (
        <CookiePreferencesDialog
          analyticsChoice={draftAnalytics}
          isOpen={isDialogOpen}
          isGpcEnabled={isGpcEnabled}
          onAnalyticsChoiceChange={setDraftAnalytics}
          onOpenChange={handleDialogOpenChange}
        />
      ) : null}
    </PrivacyConsentContext.Provider>
  );
}

function getCookieBannerCopy(region: PrivacyConsentRegion) {
  switch (region) {
    case "california":
      return {
        description:
          "We use cookies and similar technologies for site analytics and support tools. Non-essential tracking stays off unless you accept all, and you can change this later from the footer.",
        acceptLabel: "Accept all",
      };
    case "europe":
      return {
        description:
          "We use cookies and similar technologies for site analytics and support tools. You can accept all, reject all, or customize your choice at any time from the footer.",
        acceptLabel: "Accept all",
      };
    default:
      return {
        description:
          "We use cookies and similar technologies for site analytics and support tools. You can accept all, reject non-essential tracking, or change your choice later from the footer.",
        acceptLabel: "Accept all",
      };
  }
}

function CookieConsentBanner({
  isDialogOpen,
  region,
}: {
  isDialogOpen: boolean;
  region: PrivacyConsentRegion;
}) {
  const {
    analyticsChoice,
    isReady,
    openPreferences,
    rejectNonEssential,
    saveAnalyticsChoice,
  } = usePrivacyConsentContext();
  const bannerCopy = getCookieBannerCopy(region);

  if (!isReady || analyticsChoice !== null || isDialogOpen) {
    return null;
  }

  return (
    <div className="pointer-events-none fixed inset-x-4 bottom-4 z-[70] flex justify-center sm:justify-end">
      <div
        className={cn([
          "border-color-brand surface pointer-events-auto w-full max-w-xl rounded-md border p-5 shadow-2xl",
          "sm:p-6",
        ])}
        style={{ cursor: COOKIE_CURSOR }}
      >
        <div className="flex flex-col gap-4">
          <div className="space-y-2">
            <p className="text-fg font-mono text-xl font-semibold">Cookies.</p>
            <p className="text-fg text-sm leading-6">
              {bannerCopy.description}
            </p>
            <p className="text-fg-muted text-xs leading-5">
              See our{" "}
              <a
                href={COOKIE_POLICY_PATH}
                className="underline decoration-dotted underline-offset-3"
              >
                Cookie Policy
              </a>{" "}
              and{" "}
              <a
                href={PRIVACY_POLICY_PATH}
                className="underline decoration-dotted underline-offset-3"
              >
                Privacy Policy
              </a>
              .
            </p>
          </div>

          <div className="flex flex-col gap-2 sm:flex-row sm:justify-end">
            {region === "europe" ? (
              <>
                <Button
                  className={MUTED_ACTION_BUTTON_CLASS}
                  variant="ghost"
                  onClick={() => saveAnalyticsChoice(true)}
                >
                  Accept all
                </Button>
                <Button
                  className={MUTED_ACTION_BUTTON_CLASS}
                  variant="ghost"
                  onClick={rejectNonEssential}
                >
                  Reject all
                </Button>
                <Button
                  className={PRIMARY_ACTION_BUTTON_CLASS}
                  onClick={openPreferences}
                >
                  Customize
                </Button>
              </>
            ) : null}
            {region === "default" ? (
              <>
                <Button
                  className={MUTED_ACTION_BUTTON_CLASS}
                  variant="ghost"
                  onClick={rejectNonEssential}
                >
                  Reject non-essential
                </Button>
                <Button
                  className={MUTED_ACTION_BUTTON_CLASS}
                  variant="ghost"
                  onClick={openPreferences}
                >
                  Manage choices
                </Button>
                <Button
                  className={PRIMARY_ACTION_BUTTON_CLASS}
                  onClick={() => saveAnalyticsChoice(true)}
                >
                  {bannerCopy.acceptLabel}
                </Button>
              </>
            ) : null}
            {region === "california" ? (
              <Button
                className={PRIMARY_ACTION_BUTTON_CLASS}
                onClick={() => saveAnalyticsChoice(true)}
              >
                {bannerCopy.acceptLabel}
              </Button>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}

function CookiePreferencesDialog({
  analyticsChoice,
  isOpen,
  isGpcEnabled,
  onAnalyticsChoiceChange,
  onOpenChange,
}: {
  analyticsChoice: boolean;
  isOpen: boolean;
  isGpcEnabled: boolean;
  onAnalyticsChoiceChange: (value: boolean) => void;
  onOpenChange: (value: boolean) => void;
}) {
  const { rejectNonEssential, saveAnalyticsChoice } =
    usePrivacyConsentContext();

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent
        className="border-color-brand surface max-w-xl rounded-md p-0"
        style={{ cursor: COOKIE_CURSOR }}
      >
        <div className="space-y-5 p-6">
          <DialogHeader>
            <DialogTitle className="font-mono text-2xl text-neutral-900">
              Cookie preferences
            </DialogTitle>
          </DialogHeader>

          <label className="flex items-start gap-3 text-sm text-neutral-700">
            <Checkbox
              checked={isGpcEnabled ? false : analyticsChoice}
              disabled={isGpcEnabled}
              onCheckedChange={(checked) =>
                onAnalyticsChoiceChange(checked === true)
              }
              className="mt-0.5 border-neutral-300 data-[state=checked]:border-stone-600 data-[state=checked]:bg-stone-600"
            />
            <span className="space-y-1">
              <span className="block">Analytics and support tools</span>
              <span className="block text-sm leading-6 text-neutral-500">
                Includes PostHog and the website support widget.
              </span>
            </span>
          </label>

          {isGpcEnabled && (
            <div className="text-sm leading-6 text-emerald-900/85">
              Global Privacy Control is enabled in your browser, so this stays
              off until that signal is turned off.
            </div>
          )}

          <div className="text-sm leading-6 text-neutral-500">
            For details, review our{" "}
            <a
              href={COOKIE_POLICY_PATH}
              className="underline decoration-dotted underline-offset-3"
              onClick={() => onOpenChange(false)}
            >
              Cookie Policy
            </a>
            .
          </div>

          <DialogFooter>
            <Button
              className={MUTED_ACTION_BUTTON_CLASS}
              variant="ghost"
              onClick={rejectNonEssential}
            >
              Reject all
            </Button>
            <Button
              className={PRIMARY_ACTION_BUTTON_CLASS}
              onClick={() => saveAnalyticsChoice(analyticsChoice)}
            >
              Save preferences
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}
