import type { ProviderKind } from "./providers-model";

// Desktop's remote model catalog, limited to models usable after recording.
const TRANSCRIPTION_MODELS: Record<string, readonly string[]> = {
  deepgram: ["nova-3-general", "nova-3-medical"],
  assemblyai: ["universal-3-5-pro"],
  openai: ["gpt-transcribe", "gpt-4o-transcribe-diarize", "whisper-1"],
  openrouter: [
    "openai/gpt-transcribe",
    "mistralai/voxtral-mini-transcribe",
    "openai/whisper-large-v3-turbo",
    "openai/whisper-large-v3",
    "fish-audio/transcribe-1",
    "x-ai/grok-stt-1.0",
    "deepgram/nova-3",
    "microsoft/mai-transcribe-1.5",
    "nvidia/parakeet-tdt-0.6b-v3",
    "qwen/qwen3-asr-flash-2026-02-10",
    "qwen/qwen3-asr-1.7b",
    "qwen/qwen3-asr-0.6b",
    "nvidia/nemotron-3.5-asr-streaming-multilingual-0.6b",
    "mistralai/voxtral-small-24b-2507-stt",
    "mistralai/voxtral-mini-3b-2507",
    "google/chirp-3",
    "openai/whisper-1",
  ],
  zai: ["glm-asr-2512"],
  siliconflow: ["FunAudioLLM/SenseVoiceSmall", "TeleAI/TeleSpeechASR"],
  google_generative_ai: ["gemini-3.5-transcribe"],
  google_cloud: ["latest_long"],
  aws_transcribe: ["amazon-transcribe"],
  azure_speech: ["fast-transcription"],
  elevenlabs: ["scribe_v2"],
  soniox: ["stt-async-v5"],
  speechmatics: ["enhanced", "standard"],
  groq: ["whisper-large-v3-turbo", "whisper-large-v3"],
  mistral: ["voxtral-mini-2602"],
  revai: ["machine"],
  gladia: ["solaria-3", "solaria-1"],
  cartesia: ["ink-2"],
  cloudflare_workers_ai: ["nova-3"],
  together: [
    "openai/whisper-large-v3",
    "nvidia/parakeet-tdt-0.6b-v3",
    "nvidia/nemotron-3.5-asr-streaming-0.6b",
    "nvidia/nemotron-3-asr-streaming-0.6b",
  ],
  xai: ["xai-stt"],
  smallestai: ["pulse", "pulse-pro"],
  pyannote: ["parakeet-tdt-0.6b-v3", "faster-whisper-large-v3-turbo"],
  cohere: ["cohere-transcribe-03-2026", "cohere-transcribe-arabic-07-2026"],
  aquavoice: ["avalon-v1.5"],
  custom: [],
};

const SUMMARY_MODELS: Record<string, readonly string[]> = {
  google_vertex_ai: [
    "google/gemini-3.8-flash",
    "google/gemini-3.7-flash",
    "google/gemini-3.6-flash",
    "google/gemini-3.5-flash-lite",
    "google/gemini-3.1-pro-preview",
    "google/gemini-3.5-flash",
    "google/gemini-3.1-flash-lite",
  ],
  cloudflare_workers_ai: [
    "@cf/moonshotai/kimi-k2.7-code",
    "@cf/zai-org/glm-5.2",
    "@cf/moonshotai/kimi-k2.6",
    "@cf/zai-org/glm-4.7-flash",
    "@cf/openai/gpt-oss-120b",
    "@cf/meta/llama-4-scout-17b-16e-instruct",
    "@cf/google/gemma-4-26b-a4b-it",
    "@cf/nvidia/nemotron-3-120b-a12b",
    "@cf/openai/gpt-oss-20b",
    "@cf/qwen/qwen3-30b-a3b-fp8",
    "@cf/mistralai/mistral-small-3.1-24b-instruct",
    "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
  ],
};

export function presetProviderModels(kind: ProviderKind, provider: string) {
  return (kind === "stt" ? TRANSCRIPTION_MODELS : SUMMARY_MODELS)[provider];
}

export function modelOptions(models: readonly string[], selected: string) {
  return [...new Set([...models, selected].filter(Boolean))];
}
