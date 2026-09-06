const PROVIDER_ICONS: Record<string, { light: number; dark: number }> = {
  anarlog: {
    light: require("../../assets/providers/anarlog.png"),
    dark: require("../../assets/providers/anarlog.png"),
  },
  deepgram: {
    light: require("../../assets/providers/deepgram-light.png"),
    dark: require("../../assets/providers/deepgram-dark.png"),
  },
  assemblyai: {
    light: require("../../assets/providers/assemblyai-light.png"),
    dark: require("../../assets/providers/assemblyai-light.png"),
  },
  openai: {
    light: require("../../assets/providers/openai-light.png"),
    dark: require("../../assets/providers/openai-dark.png"),
  },
  openrouter: {
    light: require("../../assets/providers/openrouter-light.png"),
    dark: require("../../assets/providers/openrouter-light.png"),
  },
  zai: {
    light: require("../../assets/providers/zai-light.png"),
    dark: require("../../assets/providers/zai-dark.png"),
  },
  siliconflow: {
    light: require("../../assets/providers/siliconcloud-light.png"),
    dark: require("../../assets/providers/siliconcloud-light.png"),
  },
  google_generative_ai: {
    light: require("../../assets/providers/gemini-light.png"),
    dark: require("../../assets/providers/gemini-light.png"),
  },
  google_cloud: {
    light: require("../../assets/providers/googlecloud-light.png"),
    dark: require("../../assets/providers/googlecloud-light.png"),
  },
  aws_transcribe: {
    light: require("../../assets/providers/aws-light.png"),
    dark: require("../../assets/providers/aws-light.png"),
  },
  azure_speech: {
    light: require("../../assets/providers/azure-light.png"),
    dark: require("../../assets/providers/azure-light.png"),
  },
  elevenlabs: {
    light: require("../../assets/providers/elevenlabs-light.png"),
    dark: require("../../assets/providers/elevenlabs-dark.png"),
  },
  soniox: {
    light: require("../../assets/providers/soniox-light.png"),
    dark: require("../../assets/providers/soniox-dark.png"),
  },
  speechmatics: {
    light: require("../../assets/providers/speechmatics-light.png"),
    dark: require("../../assets/providers/speechmatics-dark.png"),
  },
  groq: {
    light: require("../../assets/providers/groq-light.png"),
    dark: require("../../assets/providers/groq-light.png"),
  },
  mistral: {
    light: require("../../assets/providers/mistral-light.png"),
    dark: require("../../assets/providers/mistral-light.png"),
  },
  revai: {
    light: require("../../assets/providers/revai-light.png"),
    dark: require("../../assets/providers/revai-dark.png"),
  },
  gladia: {
    light: require("../../assets/providers/gladia-light.png"),
    dark: require("../../assets/providers/gladia-dark.png"),
  },
  cartesia: {
    light: require("../../assets/providers/cartesia-light.png"),
    dark: require("../../assets/providers/cartesia-dark.png"),
  },
  cloudflare_workers_ai: {
    light: require("../../assets/providers/cloudflare-light.png"),
    dark: require("../../assets/providers/cloudflare-light.png"),
  },
  together: {
    light: require("../../assets/providers/together-light.png"),
    dark: require("../../assets/providers/together-light.png"),
  },
  xai: {
    light: require("../../assets/providers/xai-light.png"),
    dark: require("../../assets/providers/xai-dark.png"),
  },
  smallestai: {
    light: require("../../assets/providers/smallestai-light.png"),
    dark: require("../../assets/providers/smallestai-dark.png"),
  },
  pyannote: {
    light: require("../../assets/providers/pyannote-light.png"),
    dark: require("../../assets/providers/pyannote-dark.png"),
  },
  cohere: {
    light: require("../../assets/providers/cohere-light.png"),
    dark: require("../../assets/providers/cohere-light.png"),
  },
  aquavoice: {
    light: require("../../assets/providers/aquavoice-light.png"),
    dark: require("../../assets/providers/aquavoice-dark.png"),
  },
  anthropic: {
    light: require("../../assets/providers/anthropic-light.png"),
    dark: require("../../assets/providers/anthropic-dark.png"),
  },
  moonshot: {
    light: require("../../assets/providers/moonshot-light.png"),
    dark: require("../../assets/providers/moonshot-dark.png"),
  },
  deepseek: {
    light: require("../../assets/providers/deepseek-light.png"),
    dark: require("../../assets/providers/deepseek-light.png"),
  },
  dashscope: {
    light: require("../../assets/providers/alibabacloud-light.png"),
    dark: require("../../assets/providers/alibabacloud-light.png"),
  },
  alibaba_cloud: {
    light: require("../../assets/providers/alibabacloud-light.png"),
    dark: require("../../assets/providers/alibabacloud-light.png"),
  },
  amazon_bedrock: {
    light: require("../../assets/providers/aws-light.png"),
    dark: require("../../assets/providers/aws-light.png"),
  },
  azure_openai: {
    light: require("../../assets/providers/azure-light.png"),
    dark: require("../../assets/providers/azure-light.png"),
  },
  google_vertex_ai: {
    light: require("../../assets/providers/googlecloud-light.png"),
    dark: require("../../assets/providers/googlecloud-light.png"),
  },
  azure_ai: {
    light: require("../../assets/providers/azureai-light.png"),
    dark: require("../../assets/providers/azureai-light.png"),
  },
  fireworks: {
    light: require("../../assets/providers/fireworks-light.png"),
    dark: require("../../assets/providers/fireworks-light.png"),
  },
  cerebras: {
    light: require("../../assets/providers/cerebras-light.png"),
    dark: require("../../assets/providers/cerebras-light.png"),
  },
};

export function providerIconSource(provider: string, scheme: "light" | "dark") {
  return PROVIDER_ICONS[provider]?.[scheme];
}
