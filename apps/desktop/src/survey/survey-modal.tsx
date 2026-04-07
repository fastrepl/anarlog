import { isTauri } from "@tauri-apps/api/core";
import { CheckIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { cn } from "@hypr/utils";

import { commands } from "~/types/tauri.gen";

const SURVEY_ID = "onboarding_survey_v1";

interface SurveyQuestion {
  id: string;
  question: string;
  options: string[];
  multiSelect: boolean;
}

const SURVEY_QUESTIONS: SurveyQuestion[] = [
  {
    id: "how_found_us",
    question: "How did you find us?",
    options: [
      "Google search",
      "GitHub",
      "AI search (ChatGPT, Perplexity, etc.)",
      "Friend / colleague",
      "Other",
    ],
    multiSelect: false,
  },
  {
    id: "why_char",
    question: "Why did you choose Char over other options?",
    options: [
      "I want my data stored locally / privacy matters",
      "I want to choose my own AI provider",
      "It's open source",
      "I was looking for a free AI meeting tool",
      "Other",
    ],
    multiSelect: true,
  },
  {
    id: "role",
    question: "What's your role?",
    options: [
      "Engineer / Developer",
      "Founder / Technical leader",
      "Legal / Healthcare / Finance",
      "Product / Design / Ops",
      "Other",
    ],
    multiSelect: false,
  },
  {
    id: "current_note_taking",
    question: "How are you currently taking notes?",
    options: [
      "I'm not / pen & paper",
      "Manually in an app (Apple Notes, Notion, Google Docs, etc.)",
      "AI tool that joins the call (Otter, Fireflies, etc.)",
      "AI tool without a bot (Granola, Jamie, etc.)",
      "Other",
    ],
    multiSelect: true,
  },
];

function OptionButton({
  label,
  selected,
  onClick,
}: {
  label: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn([
        "flex items-center gap-2.5 rounded-lg border px-3.5 py-2.5 text-left text-sm transition-all",
        selected
          ? "border-neutral-900 bg-neutral-900 text-white"
          : "border-neutral-200 bg-white text-neutral-700 hover:border-neutral-300 hover:bg-neutral-50",
      ])}
    >
      <span
        className={cn([
          "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-all",
          selected
            ? "border-white bg-white"
            : "border-neutral-300 bg-transparent",
        ])}
      >
        {selected && <CheckIcon size={10} className="text-neutral-900" />}
      </span>
      <span>{label}</span>
    </button>
  );
}

function QuestionStep({
  question,
  responses,
  onToggle,
}: {
  question: SurveyQuestion;
  responses: string[];
  onToggle: (option: string) => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-base font-medium text-neutral-900">
        {question.question}
      </h3>
      {question.multiSelect && (
        <p className="text-xs text-neutral-500">Select all that apply</p>
      )}
      <div className="flex flex-col gap-2">
        {question.options.map((option) => (
          <OptionButton
            key={option}
            label={option}
            selected={responses.includes(option)}
            onClick={() => onToggle(option)}
          />
        ))}
      </div>
    </div>
  );
}

export function SurveyModal({
  forceOpen,
  onClose,
}: { forceOpen?: boolean; onClose?: () => void } = {}) {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState(0);
  const [responses, setResponses] = useState<Record<string, string[]>>({});
  const hasChecked = useRef(false);

  useEffect(() => {
    if (forceOpen) {
      setOpen(true);
      setStep(0);
      setResponses({});
      return;
    }

    if (hasChecked.current || !isTauri() || getCurrentWebviewWindowLabel() !== "main") {
      return;
    }
    hasChecked.current = true;

    void (async () => {
      const countResult = await commands.incrementAppOpenCount();
      if (countResult.status !== "ok") {
        return;
      }

      const dismissedResult = await commands.getSurveyDismissed();
      if (dismissedResult.status !== "ok") {
        return;
      }

      if (countResult.data >= 2 && !dismissedResult.data) {
        setOpen(true);
      }
    })();
  }, [forceOpen]);

  const currentQuestion = SURVEY_QUESTIONS[step];
  const currentResponses = responses[currentQuestion.id] ?? [];
  const isLastStep = step === SURVEY_QUESTIONS.length - 1;
  const canProceed = currentResponses.length > 0;

  const handleToggle = useCallback(
    (option: string) => {
      setResponses((prev) => {
        const questionId = currentQuestion.id;
        const current = prev[questionId] ?? [];

        if (currentQuestion.multiSelect) {
          const next = current.includes(option)
            ? current.filter((o) => o !== option)
            : [...current, option];
          return { ...prev, [questionId]: next };
        }

        return { ...prev, [questionId]: [option] };
      });
    },
    [currentQuestion],
  );

  const handleSubmit = useCallback(async () => {
    const payload: Record<string, unknown> = {
      event: "survey sent",
      $survey_id: SURVEY_ID,
    };

    SURVEY_QUESTIONS.forEach((q, i) => {
      const answer = responses[q.id] ?? [];
      const key = i === 0 ? "$survey_response" : `$survey_response_${i}`;
      payload[key] = q.multiSelect ? answer : (answer[0] ?? "");
    });

    void analyticsCommands.event(
      payload as Parameters<typeof analyticsCommands.event>[0],
    );

    void commands.setSurveyDismissed(true);
    setOpen(false);
    onClose?.();
  }, [responses, onClose]);

  const handleDismiss = useCallback(() => {
    void analyticsCommands.event({
      event: "survey dismissed",
      $survey_id: SURVEY_ID,
    });
    void commands.setSurveyDismissed(true);
    setOpen(false);
    onClose?.();
  }, [onClose]);

  const handleNext = useCallback(() => {
    if (isLastStep) {
      void handleSubmit();
    } else {
      setStep((s) => s + 1);
    }
  }, [isLastStep, handleSubmit]);

  const handleBack = useCallback(() => {
    if (step > 0) {
      setStep((s) => s - 1);
    }
  }, [step]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && handleDismiss()}>
      <DialogContent className="max-w-md gap-0 p-0">
        <DialogHeader className="px-6 pt-6 pb-2">
          <DialogTitle className="text-lg font-semibold">
            Quick survey
          </DialogTitle>
          <DialogDescription className="text-sm text-neutral-500">
            Help us make Char better for you
          </DialogDescription>
        </DialogHeader>

        <div className="px-6 pt-2 pb-4">
          <QuestionStep
            question={currentQuestion}
            responses={currentResponses}
            onToggle={handleToggle}
          />
        </div>

        <DialogFooter className="flex items-center border-t px-6 py-4">
          <div className="flex w-full items-center justify-between">
            <div className="flex items-center gap-1.5">
              {SURVEY_QUESTIONS.map((_, i) => (
                <span
                  key={i}
                  className={cn([
                    "h-1.5 w-1.5 rounded-full transition-colors",
                    i === step ? "bg-neutral-900" : "bg-neutral-300",
                  ])}
                />
              ))}
            </div>
            <div className="flex items-center gap-2">
              {step > 0 && (
                <Button variant="ghost" size="sm" onClick={handleBack}>
                  Back
                </Button>
              )}
              <Button size="sm" onClick={handleNext} disabled={!canProceed}>
                {isLastStep ? "Submit" : "Next"}
              </Button>
            </div>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
