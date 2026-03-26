import levenshtein from "js-levenshtein-esm";

import type { EnhanceTemplate } from "@hypr/plugin-template";

import type { EarlyValidatorFn } from "~/store/zustand/ai-task/shared/validate";

export function normalizeForComparison(text: string): string {
  return text
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9\s]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

type PipeStep = (
  text: string,
) => { text: string } | { valid: true } | { valid: false; feedback: string };

function pipe(...fns: PipeStep[]): EarlyValidatorFn {
  return (text) => {
    let current = text;
    for (const fn of fns) {
      const result = fn(current);
      if ("valid" in result && !result.valid) return result;
      if ("text" in result) current = result.text;
    }
    return { valid: true };
  };
}

function stripPreamble(): (text: string) => { text: string } {
  return (text) => {
    const idx = text.indexOf("#");
    if (idx > 0) {
      return { text: text.slice(idx) };
    }
    return { text };
  };
}

function requireH1(): EarlyValidatorFn {
  return (text) => {
    if (!text.trim().startsWith("# ")) {
      return {
        valid: false,
        feedback: "Output must start with a markdown h1 heading (# Title).",
      };
    }
    return { valid: true };
  };
}

function matchSectionHeading(title: string): EarlyValidatorFn {
  const expectedStart = `# ${title}`;
  const expectedNormalized = normalizeForComparison(title);

  return (text) => {
    const trimmed = text.trim();

    if (
      expectedStart.startsWith(trimmed) ||
      trimmed.startsWith(expectedStart)
    ) {
      return { valid: true };
    }

    const actualNormalized = normalizeForComparison(trimmed.slice(2));
    const distance = levenshtein(expectedNormalized, actualNormalized);
    const threshold = Math.floor(expectedNormalized.length * 0.3);
    if (distance <= threshold) {
      return { valid: true };
    }

    return {
      valid: false,
      feedback: `Output must start with the first template section heading: "${expectedStart}"`,
    };
  };
}

export function createEnhanceValidator(
  template: EnhanceTemplate | null,
): EarlyValidatorFn {
  const steps: PipeStep[] = [stripPreamble(), requireH1()];

  if (template?.sections?.length) {
    steps.push(matchSectionHeading(template.sections[0].title));
  }

  return pipe(...steps);
}
