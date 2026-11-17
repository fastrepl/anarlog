import * as icons from "@lobehub/icons";
// Renders the @lobehub/icons provider logos shipping shows in AI settings (`ProviderLobeIcon` in
// apps/desktop/src/settings/ai/shared) to SVGs: the color variant when there is one, the brand
// color unless it is black or white, else `currentColor` so the app tints it. The package needs
// a bundler; from the repository root with dependencies installed:
// NODE_PATH=apps/desktop/node_modules npx esbuild apps/desktop-gpui/src/product/export-provider-icons.mjs \
//   --bundle --platform=node --format=cjs --outfile=/tmp/provider-icons.cjs
// node /tmp/provider-icons.cjs apps/desktop-gpui/src/product/icons/providers
import { writeFileSync } from "node:fs";
import path from "node:path";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

const PROVIDERS = {
  alibaba_cloud: "AlibabaCloud",
  amazon_bedrock: "Bedrock",
  anthropic: "Anthropic",
  apple_foundation: "Apple",
  apple_speech: "Apple",
  assemblyai: "AssemblyAI",
  aws_transcribe: "Aws",
  azure_ai: "AzureAI",
  azure_openai: "Azure",
  azure_speech: "Azure",
  cerebras: "Cerebras",
  chatgpt: "OpenAI",
  claude: "Claude",
  cloudflare_workers_ai: "WorkersAI",
  cohere: "Cohere",
  cursor: "Cursor",
  dashscope: "AlibabaCloud",
  deepseek: "DeepSeek",
  elevenlabs: "ElevenLabs",
  fireworks: "Fireworks",
  github_copilot: "GithubCopilot",
  google_cloud: "GoogleCloud",
  google_generative_ai: "Gemini",
  google_vertex_ai: "VertexAI",
  grok: "Grok",
  groq: "Groq",
  kimi_code: "Moonshot",
  lmstudio: "LmStudio",
  mistral: "Mistral",
  moonshot: "Moonshot",
  nvidia: "Nvidia",
  ollama: "Ollama",
  openai: "OpenAI",
  openrouter: "OpenRouter",
  siliconflow: "SiliconCloud",
  together: "Together",
  xai: "XAI",
  zai: "ZAI",
};
const TINTED = new Set(["#000", "#000000", "#fff", "#ffffff"]);

for (const [id, name] of Object.entries(PROVIDERS)) {
  const Icon = icons[name];
  const brand = Icon.colorPrimary?.toLowerCase();
  const element = Icon.Color
    ? React.createElement(Icon.Color, { size: 24 })
    : brand && !TINTED.has(brand)
      ? React.createElement(Icon, { size: 24, color: Icon.colorPrimary })
      : React.createElement(Icon, { size: 24 });
  const markup = renderToStaticMarkup(element)
    .replace(/<title>.*?<\/title>/, "")
    .replace(/ style="[^"]*"/g, "");
  writeFileSync(path.join(process.argv[2], `${id}.svg`), `${markup}\n`);
}
