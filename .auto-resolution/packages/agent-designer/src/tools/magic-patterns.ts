import { tool } from "@langchain/core/tools";
import type { LangGraphRunnableConfig } from "@langchain/langgraph";
import { z } from "zod";

import { env } from "../env";

const MagicPatternsMode = z.enum(["fast", "best"]);
const MagicPatternsModelSelector = z.enum(["auto", "claude_sonnet", "gemini"]);

const magicPatternsArgsSchema = z.object({
  prompt: z.string().describe("The prompt describing the design to create"),
  mode: MagicPatternsMode.optional()
    .default("fast")
    .describe(
      "The mode to use: 'fast' for quicker generation or 'best' for higher quality",
    ),
  presetId: z
    .string()
    .optional()
    .describe(
      "Preset configuration ID. Options: 'html-tailwind', 'shadcn-tailwind', 'chakraUi-tailwind', 'mantine-tailwind', or a custom configuration ID",
    ),
  modelSelector: MagicPatternsModelSelector.optional()
    .default("auto")
    .describe(
      "The model to use for generation: 'auto', 'claude_sonnet', or 'gemini'",
    ),
});

export type MagicPatternsArgs = z.infer<typeof magicPatternsArgsSchema>;

interface SourceFile {
  id: string;
  name: string;
  code: string;
  type: "javascript" | "css" | "asset";
}

interface CompiledFile {
  id: string;
  fileName: string;
  hostedUrl: string;
  type: "javascript" | "css" | "font";
}

interface ChatMessage {
  role: string;
  content: string;
}

interface MagicPatternsResponse {
  id: string;
  sourceFiles: SourceFile[];
  compiledFiles: CompiledFile[];
  editorUrl: string;
  previewUrl: string;
  chatMessages: ChatMessage[];
}

export const magicPatternsTool = tool(
  async (
    { prompt, mode, presetId, modelSelector }: MagicPatternsArgs,
    config: LangGraphRunnableConfig,
  ) => {
    config.writer?.({
      type: "subgraph",
      name: "magic-patterns",
      task: `Creating design: ${prompt}`,
    });

    if (!env.MAGIC_PATTERNS_API_KEY) {
      return "Error: MAGIC_PATTERNS_API_KEY is not configured";
    }

    const formData = new FormData();
    formData.append("prompt", prompt);
    if (mode) formData.append("mode", mode);
    if (presetId) formData.append("presetId", presetId);
    if (modelSelector) formData.append("modelSelector", modelSelector);

    const response = await fetch(
      "https://api.magicpatterns.com/api/v2/pattern",
      {
        method: "POST",
        headers: {
          "x-mp-api-key": env.MAGIC_PATTERNS_API_KEY,
        },
        body: formData,
      },
    );

    if (!response.ok) {
      const errorText = await response.text();
      return `Failed to create design: ${response.status} ${response.statusText} - ${errorText}`;
    }

    const result: MagicPatternsResponse = await response.json();

    const sourceFileSummary = result.sourceFiles
      .map((f) => `- ${f.name} (${f.type})`)
      .join("\n");

    return `Design created successfully!

ID: ${result.id}
Editor URL: ${result.editorUrl}
Preview URL: ${result.previewUrl}

Source Files:
${sourceFileSummary}

${result.sourceFiles.map((f) => `### ${f.name}\n\`\`\`${f.type === "javascript" ? "tsx" : f.type}\n${f.code}\n\`\`\``).join("\n\n")}`;
  },
  {
    name: "magicPatterns",
    description:
      "Create a new UI design using Magic Patterns. Provide a prompt describing the design you want (e.g., 'Create a login page', 'Build a dashboard with charts'). Returns the generated source code and URLs to preview/edit the design.",
    schema: magicPatternsArgsSchema,
  },
);
