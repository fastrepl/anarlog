// Must stay in sync with SELECTABLE_MODELS in crates/llm-proxy/src/model.rs;
// the proxy routes any other id to its task default.
export const ANARLOG_PRO_LLM_MODELS = [
  { id: "Auto", label: "Pro (Cloud)" },
  { id: "~anthropic/claude-sonnet-latest", label: "Claude Sonnet" },
  { id: "~anthropic/claude-opus-latest", label: "Claude Opus" },
  { id: "~openai/gpt-sol-latest", label: "GPT Sol" },
  { id: "~google/gemini-pro-latest", label: "Gemini Pro" },
  { id: "~google/gemini-flash-latest", label: "Gemini Flash" },
] as const;

export function anarlogProLlmModelLabel(model: string): string | undefined {
  return ANARLOG_PRO_LLM_MODELS.find(({ id }) => id === model)?.label;
}

export function isAnarlogProLlmModel(model: string): boolean {
  return ANARLOG_PRO_LLM_MODELS.some(({ id }) => id === model);
}
