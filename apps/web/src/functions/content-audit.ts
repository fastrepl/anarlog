const CONTENT_AUDIT_MODEL =
  process.env.CONTENT_AUDIT_MODEL || "openai/gpt-4o-mini";

const CONTENT_AUDIT_SYSTEM_PROMPT = `You are auditing a technical blog post draft before publication.

Rewrite the markdown so it reads like a sharp human editor revised it manually.

Apply these rules:
- Remove filler, throat-clearing, rhetorical scaffolding, binary contrast setups, dramatic fragments, and generic conclusions.
- Remove AI-sounding phrasing, inflated significance, vague attribution, empty intensifiers, promotional language, and formulaic rhythm.
- Tighten wording, improve cadence, and fix grammar.
- Keep the voice direct, technical, and concrete.
- Preserve the author's intent, structure, headings, lists, markdown, links, code blocks, quotes, and factual claims.
- Do not invent facts, citations, anecdotes, or product claims.
- Do not turn concise copy into longer copy.
- Return valid JSON only.

Respond with this shape:
{
  "revisedContent": "the revised markdown",
  "summary": ["3 to 6 short bullets describing the most important edits"]
}`;

interface AuditArticleContentParams {
  path: string;
  content: string;
  metadata: Record<string, unknown>;
  openrouterApiKey: string;
}

interface AuditArticleContentResult {
  success: boolean;
  revisedContent?: string;
  summary?: string[];
  changed?: boolean;
  model?: string;
  error?: string;
}

function buildMetadataContext(metadata: Record<string, unknown>) {
  const lines = [
    `Meta title: ${String(metadata.meta_title || "")}`,
    `Display title: ${String(metadata.display_title || "")}`,
    `Meta description: ${String(metadata.meta_description || "")}`,
    `Category: ${String(metadata.category || "")}`,
  ];

  const authors = Array.isArray(metadata.author)
    ? metadata.author
        .filter((value): value is string => typeof value === "string")
        .join(", ")
    : "";

  if (authors) {
    lines.push(`Author: ${authors}`);
  }

  return lines.join("\n");
}

export async function auditArticleContent({
  path,
  content,
  metadata,
  openrouterApiKey,
}: AuditArticleContentParams): Promise<AuditArticleContentResult> {
  const response = await fetch(
    "https://openrouter.ai/api/v1/chat/completions",
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${openrouterApiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model: CONTENT_AUDIT_MODEL,
        temperature: 0.2,
        response_format: { type: "json_object" },
        messages: [
          {
            role: "system",
            content: CONTENT_AUDIT_SYSTEM_PROMPT,
          },
          {
            role: "user",
            content: `Path: ${path}

Metadata:
${buildMetadataContext(metadata)}

Draft markdown:

${content}`,
          },
        ],
      }),
    },
  );

  if (!response.ok) {
    const errorText = await response.text();
    return {
      success: false,
      error: `OpenRouter API error: ${errorText}`,
    };
  }

  const data = await response.json();
  const messageContent = data.choices?.[0]?.message?.content;

  if (!messageContent || typeof messageContent !== "string") {
    return { success: false, error: "No response from LLM" };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(messageContent);
  } catch {
    return {
      success: false,
      error: `Failed to parse LLM response: ${messageContent}`,
    };
  }

  const revisedContent =
    parsed &&
    typeof parsed === "object" &&
    "revisedContent" in parsed &&
    typeof parsed.revisedContent === "string"
      ? parsed.revisedContent
      : content;

  const summary =
    parsed &&
    typeof parsed === "object" &&
    "summary" in parsed &&
    Array.isArray(parsed.summary)
      ? parsed.summary
          .filter((item): item is string => typeof item === "string")
          .slice(0, 6)
      : [];

  return {
    success: true,
    revisedContent,
    summary,
    changed: revisedContent !== content,
    model: CONTENT_AUDIT_MODEL,
  };
}
