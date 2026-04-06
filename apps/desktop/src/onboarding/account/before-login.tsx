import { CalendarIcon, Puzzle, Sparkle } from "lucide-react";

import { cn } from "@hypr/utils";

import { OnboardingButton, OnboardingCharIcon } from "../shared";

import { useAuth } from "~/auth";

const FEATURES = [
  {
    label: "Cloud Services",
    icon: Sparkle,
    benefit:
      "Get hosted transcription and language models without managing API keys.",
    accent: { icon: "text-blue-900", label: "text-blue-950" },
  },
  {
    label: "Integrations",
    icon: Puzzle,
    benefit: "Connect tools and pull context into Char with less busywork.",
    accent: { icon: "text-purple-700", label: "text-purple-900" },
  },
  {
    label: "Calendar Sync",
    icon: CalendarIcon,
    benefit:
      "Sync your Google Calendar or Microsoft Outlook to stay on top of meetings.",
    accent: { icon: "text-emerald-700", label: "text-emerald-900" },
  },
] as const;

export function BeforeLogin({ onContinue: _ }: { onContinue: () => void }) {
  const auth = useAuth();

  return (
    <div className="flex flex-col">
      <div className="mb-8 flex flex-col items-start justify-start gap-8 py-4">
        {FEATURES.map((f) => (
          <FeatureItem key={f.label} feature={f} />
        ))}
      </div>

      <div className="flex flex-col items-start">
        <div className="flex flex-row items-center gap-4">
          <OnboardingButton
            onClick={() => {
              void auth?.signIn();
            }}
            className="flex items-center gap-2 px-8 py-3 text-base"
          >
            <OnboardingCharIcon inverted />
            Get started for free
          </OnboardingButton>

          <button
            type="button"
            onClick={() => {
              void auth?.signIn();
            }}
            className="text-md rounded-full border border-neutral-300 bg-transparent px-8 py-3 font-medium text-neutral-500 transition-colors hover:text-neutral-700"
          >
            Login with existing account
          </button>
        </div>
      </div>
    </div>
  );
}

function FeatureItem({ feature }: { feature: (typeof FEATURES)[number] }) {
  const Icon = feature.icon;

  return (
    <div className="flex flex-row items-center gap-3 text-left">
      <div className="flex items-center justify-center">
        <Icon className={cn(["h-5 w-5", feature.accent.icon])} />
      </div>
      <div className="flex flex-col items-start">
        <p className={cn(["text-sm font-medium", feature.accent.label])}>
          {feature.label}
        </p>
        <p className="text-xs leading-[1.45] text-neutral-500">
          {feature.benefit}
        </p>
      </div>
    </div>
  );
}
