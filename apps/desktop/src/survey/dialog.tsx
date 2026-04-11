import { CheckIcon } from "lucide-react";
import { useRef, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { Input } from "@hypr/ui/components/ui/input";
import { cn } from "@hypr/utils";

import {
  onboardingSurveyQuestions,
  type SurveyQuestion,
  type SurveyResponses,
} from "./config";

const OPEN_CHOICE_PLACEHOLDER = "Please specify";

function getOpenChoiceOption(question: SurveyQuestion) {
  if (!question.hasOpenChoice) {
    return null;
  }

  return question.options[question.options.length - 1] ?? null;
}

function resolveQuestionResponses(
  question: SurveyQuestion,
  selectedOptions: string[],
  openChoiceValue: string,
) {
  const openChoiceOption = getOpenChoiceOption(question);
  const trimmedOpenChoiceValue = openChoiceValue.trim();

  return selectedOptions.reduce<string[]>((resolved, option) => {
    if (option !== openChoiceOption) {
      resolved.push(option);
      return resolved;
    }

    if (trimmedOpenChoiceValue.length > 0) {
      resolved.push(trimmedOpenChoiceValue);
    }

    return resolved;
  }, []);
}

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
        "flex items-center gap-2.5 rounded-lg border px-3.5 py-2.5 text-left text-sm transition-colors",
        selected
          ? "border-neutral-900 bg-neutral-900 text-white"
          : "border-neutral-200 bg-white text-neutral-700 hover:border-neutral-300 hover:bg-neutral-50",
      ])}
    >
      <span
        className={cn([
          "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors",
          selected
            ? "border-white bg-white"
            : "border-neutral-300 bg-transparent",
        ])}
      >
        {selected ? <CheckIcon size={10} className="text-neutral-900" /> : null}
      </span>
      <span>{label}</span>
    </button>
  );
}

function OpenChoiceOption({
  questionPrompt,
  value,
  placeholder,
  onChange,
  onClear,
}: {
  questionPrompt: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
  onClear: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div
      className="flex items-center gap-2.5 rounded-lg border border-neutral-900 bg-neutral-900 px-3.5 py-2.5 text-sm text-white transition-colors"
      onClick={() => inputRef.current?.focus()}
    >
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onClear();
        }}
        className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-white bg-white"
        aria-label="Clear Other response"
      >
        <CheckIcon size={10} className="text-neutral-900" />
      </button>
      <Input
        ref={inputRef}
        autoFocus
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        aria-label={`${questionPrompt} other response`}
        className="h-auto border-0 bg-transparent px-0 py-0 text-sm text-white shadow-none placeholder:text-neutral-300 focus-visible:ring-0"
      />
    </div>
  );
}

function QuestionStep({
  question,
  responses,
  openChoiceValue,
  onToggle,
  onOpenChoiceChange,
}: {
  question: SurveyQuestion;
  responses: string[];
  openChoiceValue: string;
  onToggle: (option: string) => void;
  onOpenChoiceChange: (value: string) => void;
}) {
  const openChoiceOption = getOpenChoiceOption(question);

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-base font-medium text-neutral-900">
        {question.prompt}
      </h3>
      {question.multiSelect ? (
        <p className="text-xs text-neutral-500">Select all that apply</p>
      ) : null}
      <div className="flex flex-col gap-2">
        {question.options.map((option) => {
          const isOpenChoice = option === openChoiceOption;
          const selected = responses.includes(option);

          return (
            <div key={option}>
              {isOpenChoice && selected ? (
                <OpenChoiceOption
                  questionPrompt={question.prompt}
                  value={openChoiceValue}
                  placeholder={OPEN_CHOICE_PLACEHOLDER}
                  onChange={onOpenChoiceChange}
                  onClear={() => onToggle(option)}
                />
              ) : (
                <OptionButton
                  label={option}
                  selected={selected}
                  onClick={() => onToggle(option)}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function OnboardingSurveyDialog({
  open,
  onOpenChange,
  onSubmit,
  submitting = false,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (responses: SurveyResponses) => void;
  submitting?: boolean;
}) {
  const [step, setStep] = useState(0);
  const [responses, setResponses] = useState<SurveyResponses>({});
  const [openChoiceResponses, setOpenChoiceResponses] = useState<
    Partial<Record<string, string>>
  >({});

  const currentQuestion = onboardingSurveyQuestions[step];
  const currentResponses = responses[currentQuestion.id] ?? [];
  const currentOpenChoiceValue = openChoiceResponses[currentQuestion.id] ?? "";
  const currentOpenChoiceOption = getOpenChoiceOption(currentQuestion);
  const isLastStep = step === onboardingSurveyQuestions.length - 1;
  const currentQuestionNeedsOpenChoice =
    currentOpenChoiceOption !== null &&
    currentResponses.includes(currentOpenChoiceOption) &&
    currentOpenChoiceValue.trim().length === 0;
  const canContinue =
    currentResponses.length > 0 &&
    !currentQuestionNeedsOpenChoice &&
    !submitting;

  const handleToggle = (option: string) => {
    setResponses((current) => {
      const selected = current[currentQuestion.id] ?? [];

      if (currentQuestion.multiSelect) {
        return {
          ...current,
          [currentQuestion.id]: selected.includes(option)
            ? selected.filter((value) => value !== option)
            : [...selected, option],
        };
      }

      return {
        ...current,
        [currentQuestion.id]: [option],
      };
    });
  };

  const buildSurveyResponses = (): SurveyResponses =>
    onboardingSurveyQuestions.reduce<SurveyResponses>(
      (nextResponses, question) => {
        const resolvedResponses = resolveQuestionResponses(
          question,
          responses[question.id] ?? [],
          openChoiceResponses[question.id] ?? "",
        );

        if (resolvedResponses.length > 0) {
          nextResponses[question.id] = resolvedResponses;
        }

        return nextResponses;
      },
      {},
    );

  const handlePrimary = () => {
    if (!canContinue) {
      return;
    }

    if (isLastStep) {
      onSubmit(buildSurveyResponses());
      return;
    }

    setStep((current) => current + 1);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md gap-0 p-0">
        <DialogHeader className="px-6 pt-6 pb-2">
          <DialogTitle className="text-lg font-semibold">
            Quick survey
          </DialogTitle>
          <DialogDescription className="text-sm text-neutral-500">
            Help us make Char better for you.
          </DialogDescription>
        </DialogHeader>

        <div className="px-6 pt-2 pb-4">
          <QuestionStep
            question={currentQuestion}
            responses={currentResponses}
            openChoiceValue={currentOpenChoiceValue}
            onToggle={handleToggle}
            onOpenChoiceChange={(value) =>
              setOpenChoiceResponses((current) => ({
                ...current,
                [currentQuestion.id]: value,
              }))
            }
          />
        </div>

        <DialogFooter className="border-t px-6 py-4">
          <div className="flex w-full items-center justify-between">
            <div className="flex items-center gap-1.5">
              {onboardingSurveyQuestions.map((question, index) => (
                <span
                  key={question.id}
                  className={cn([
                    "h-1.5 w-1.5 rounded-full transition-colors",
                    index === step ? "bg-neutral-900" : "bg-neutral-300",
                  ])}
                />
              ))}
            </div>
            <div className="flex items-center gap-2">
              {step > 0 ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setStep((current) => current - 1)}
                  disabled={submitting}
                >
                  Back
                </Button>
              ) : null}
              <Button size="sm" onClick={handlePrimary} disabled={!canContinue}>
                {isLastStep ? "Submit" : "Next"}
              </Button>
            </div>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
