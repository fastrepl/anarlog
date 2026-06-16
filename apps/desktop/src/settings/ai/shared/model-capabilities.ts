import { getCloudflareWorkersAIModelMetadata } from "./list-cloudflare-workers-ai";

const TEXT_ONLY_MODEL_RE =
  /(?:^|[/:\-.])(?:gpt-3\.5|claude-2|claude-instant|davinci|babbage|curie|ada|dall-e|sora|gpt-image|image-generation|embed|embedding|whisper|tts|transcribe|moderation|realtime|computer)(?:$|[/:\-.])/i;

const IMAGE_INPUT_MODEL_RE =
  /(?:gpt-4o|gpt-4\.1|gpt-5|claude-3|claude-sonnet|claude-opus|claude-haiku|gemini|pixtral|vision|vl|llava|llama-3\.2-vision|llama3\.2-vision|moondream|minicpm-v|internvl|qwen(?:2|2\.5|3)?-vl|gemma-3|gemma3)/i;

export function modelSupportsImageInput(
  providerId: string | undefined,
  modelId: string | undefined,
): boolean {
  if (!providerId || !modelId) {
    return false;
  }

  if (providerId === "cloudflare_workers_ai") {
    const metadata = getCloudflareWorkersAIModelMetadata(modelId);
    if (metadata?.input_modalities) {
      return metadata.input_modalities.includes("image");
    }
  }

  if (TEXT_ONLY_MODEL_RE.test(modelId)) {
    return false;
  }

  if (providerId === "hyprnote" && modelId === "Auto") {
    return true;
  }

  if (IMAGE_INPUT_MODEL_RE.test(modelId)) {
    return true;
  }

  return false;
}
