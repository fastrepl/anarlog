// Keep remote model modes in step with desktop's stt/capabilities.ts.
export function supportsLiveTranscription(provider: string, model: string) {
  if (!model) return false;
  switch (provider) {
    case "anarlog":
      return true;
    case "deepgram":
    case "cloudflare_workers_ai":
    case "cartesia":
    case "dashscope":
    case "xai":
      return true;
    case "assemblyai":
      return !["universal-3-pro", "universal-3-5-pro"].includes(model);
    case "openai":
      return ![
        "gpt-transcribe",
        "gpt-4o-transcribe-diarize",
        "gpt-4o-transcribe",
        "gpt-4o-mini-transcribe",
        "whisper-1",
      ].includes(model);
    case "google_generative_ai":
      return model.includes("transcribe-live");
    case "elevenlabs":
      return model !== "scribe_v2";
    case "soniox":
      return !["stt-async-v5", "stt-async-v4"].includes(model);
    case "mistral":
      return !["voxtral-mini-2602", "voxtral-mini-latest"].includes(model);
    case "gladia":
      return model !== "solaria-3";
    case "smallestai":
      return model !== "pulse-pro";
    default:
      return false;
  }
}

export function batchTranscriptionModel(provider: string, model: string) {
  if (provider === "dashscope") return null;
  if (provider === "openai" && model === "gpt-live-transcribe")
    return "gpt-transcribe";
  if (provider === "deepgram" && model.startsWith("flux-"))
    return "nova-3-general";
  if (
    provider === "assemblyai" &&
    ["u3-rt-pro", "universal-3-5-pro-realtime"].includes(model)
  )
    return "universal-3-5-pro";
  if (provider === "google_generative_ai" && model.includes("transcribe-live"))
    return model.replace("transcribe-live", "transcribe");
  if (provider === "elevenlabs" && model === "scribe_v2_realtime")
    return "scribe_v2";
  if (
    provider === "mistral" &&
    model === "voxtral-mini-transcribe-realtime-2602"
  )
    return "voxtral-mini-2602";
  if (provider === "soniox" && /^stt-(?:rt-)?v[45]$/.test(model))
    return model.replace(/^stt-(?:rt-)?/, "stt-async-");
  return model;
}
