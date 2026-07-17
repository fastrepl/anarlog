const MODEL_NAME_OVERRIDES: Record<string, string> = {
  "chat-latest": "Chat Latest",
  "gpt-chat-latest": "GPT Chat Latest",
  "claude-sonnet-latest": "Claude Sonnet 5",
  "claude-sonnet-5": "Claude Sonnet 5",
};

export function displayLlmModelId(providerId: string, model: string): string {
  if (providerId === "hyprnote" && model === "Auto") {
    return "Pro (Cloud)";
  }

  const modelId = lastModelPathSegment(model);
  const normalized = stripReleaseDate(modelId.toLowerCase());

  const override = MODEL_NAME_OVERRIDES[normalized];
  if (override) {
    return override;
  }

  const claudeName = formatClaudeModel(normalized);
  if (claudeName) {
    return claudeName;
  }

  const gptName = formatGptModel(normalized);
  if (gptName) {
    return gptName;
  }

  const geminiName = formatNamedFamily(normalized, "gemini", "Gemini");
  if (geminiName) {
    return geminiName;
  }

  const mistralName = formatMistralModel(normalized);
  if (mistralName) {
    return mistralName;
  }

  const kimiName = formatNamedFamily(normalized, "kimi", "Kimi");
  if (kimiName) {
    return kimiName;
  }

  return titleizeModelId(normalized);
}

function lastModelPathSegment(model: string) {
  return (
    model.trim().replace(/^~/, "").split("/").filter(Boolean).pop() ?? model
  );
}

function stripReleaseDate(modelId: string) {
  return modelId.replace(/-(?:20\d{6}|20\d{2}-\d{2}-\d{2}|2\d{3})$/, "");
}

function formatClaudeModel(modelId: string) {
  const match = modelId.match(/^claude-(opus|sonnet|haiku|fable)-(.+)$/);
  if (!match) {
    return null;
  }

  return `Claude ${capitalize(match[1])} ${formatVersion(match[2])}`;
}

function formatGptModel(modelId: string) {
  const match = modelId.match(/^gpt-(.+)$/);
  if (!match) {
    return null;
  }

  return `GPT ${formatVersion(match[1])}`;
}

function formatMistralModel(modelId: string) {
  for (const family of ["mistral", "voxtral", "ministral", "magistral"]) {
    const name = formatNamedFamily(modelId, family, capitalize(family));
    if (name) {
      return name;
    }
  }

  return null;
}

function formatNamedFamily(
  modelId: string,
  family: string,
  displayFamily: string,
) {
  const match = modelId.match(new RegExp(`^${family}-(.+)$`));
  if (!match) {
    return null;
  }

  return `${displayFamily} ${formatVersion(match[1])}`;
}

function formatVersion(value: string) {
  return value
    .replace(/-/g, " ")
    .replace(/\b(\d+) (\d+)\b/g, "$1.$2")
    .split(" ")
    .filter((part) => part !== "latest")
    .map(formatToken)
    .join(" ");
}

function titleizeModelId(modelId: string) {
  return modelId.replace(/[-_]/g, " ").split(" ").map(formatToken).join(" ");
}

function formatToken(token: string) {
  if (!token) {
    return token;
  }

  const upperTokens = new Set(["api", "oss", "vl", "llm", "ai"]);
  if (upperTokens.has(token)) {
    return token.toUpperCase();
  }

  return token.replace(/\b[a-z]/g, (letter) => letter.toUpperCase());
}

function capitalize(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}
