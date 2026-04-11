import type { AnalyticsPayload } from "@hypr/plugin-analytics";

export const ONBOARDING_SURVEY_ID = "019d7b82-451a-0000-e4c8-3a53dd3f2435";

export type SurveyQuestion = {
  id: string;
  prompt: string;
  options: string[];
  multiSelect: boolean;
  hasOpenChoice?: boolean;
};

export type SurveyResponses = Partial<Record<string, string[]>>;

export const onboardingSurveyQuestions: SurveyQuestion[] = [
  {
    id: "7405e454-508b-4e2a-80a7-e04e84f0bbaa",
    prompt: "How did you find us?",
    options: [
      "Search engine (Google, Bing, etc)",
      "AI assistant (like ChatGPT)",
      "Social media (X, LinkedIn, YouTube, etc)",
      "Referral from a friend or colleague",
      "GitHub",
      "Other",
    ],
    multiSelect: false,
    hasOpenChoice: true,
  },
  {
    id: "a9b47320-a169-4f93-90e6-15375fed4e8d",
    prompt: "Why did you decide to use Char?",
    options: [
      "I want my data stored locally / privacy matters",
      "I want to choose my own AI provider",
      "It's open source",
      "I was looking for a free AI meeting tool",
      "Other",
    ],
    multiSelect: true,
    hasOpenChoice: true,
  },
  {
    id: "ce59d931-ed0d-4d23-9ab5-55656a1e638a",
    prompt: "What best describes your role?",
    options: [
      "Engineer / Developer",
      "Founder / Executive",
      "Product",
      "Design",
      "Operations",
      "Sales / Marketing / Customer Success",
      "Research / Education / Student",
      "Other",
    ],
    multiSelect: false,
    hasOpenChoice: true,
  },
  {
    id: "d58b62d0-c689-4c72-877d-fa949b30ca47",
    prompt: "How have you been taking notes?",
    options: [
      "I'm not / pen & paper",
      "Manually in an app (Apple Notes, Notion, Google Docs, etc.)",
      "AI tool that joins the call (Otter, Fireflies, etc.)",
      "AI tool without a bot (Granola, Jamie, etc.)",
      "Other",
    ],
    multiSelect: true,
    hasOpenChoice: true,
  },
];

function surveyResponseKey(question: SurveyQuestion) {
  return `$survey_response_${question.id}`;
}

function formatSurveyResponse(question: SurveyQuestion, answers: string[]) {
  if (!question.multiSelect) {
    return answers[0] ?? "";
  }

  return answers;
}

function buildSurveyQuestionsPayload(responses: SurveyResponses) {
  return onboardingSurveyQuestions.map((question) => ({
    id: question.id,
    question: question.prompt,
    response: formatSurveyResponse(question, responses[question.id] ?? []),
  }));
}

export function buildOnboardingSurveySubmittedPayload(
  responses: SurveyResponses,
): AnalyticsPayload {
  const payload: AnalyticsPayload = {
    event: "survey sent",
    $survey_id: ONBOARDING_SURVEY_ID,
    $survey_questions: buildSurveyQuestionsPayload(responses),
  };

  onboardingSurveyQuestions.forEach((question) => {
    payload[surveyResponseKey(question)] = formatSurveyResponse(
      question,
      responses[question.id] ?? [],
    );
  });

  return payload;
}

export function buildOnboardingSurveyShownPayload(): AnalyticsPayload {
  return {
    event: "survey shown",
    $survey_id: ONBOARDING_SURVEY_ID,
  };
}

export function buildOnboardingSurveyDismissedPayload(): AnalyticsPayload {
  return {
    event: "survey dismissed",
    $survey_id: ONBOARDING_SURVEY_ID,
  };
}
