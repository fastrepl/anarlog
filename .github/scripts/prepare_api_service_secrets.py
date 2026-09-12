"""Write only the selected API service's runtime secrets; never print values."""

import argparse
import json
from pathlib import Path

SHARED = {
    "SUPABASE_URL",
    "SUPABASE_ANON_KEY",
    "SUPABASE_SERVICE_ROLE_KEY",
    "SENTRY_DSN",
    "POSTHOG_API_KEY",
    "RUST_LOG",
    "OTEL_SERVICE_NAME",
    "OTEL_EXPORTER_OTLP_ENDPOINT",
    "HONEYCOMB_API_KEY",
    "HONEYCOMB_API_ENDPOINT",
    "HONEYCOMB_DATASET",
    "HONEYCOMB_UI_BASE_URL",
    "HONEYCOMB_UI_TEAM",
    "HONEYCOMB_UI_ENVIRONMENT",
}
AI = {
    "OPENROUTER_API_KEY",
    "API_BASE_URL",
    "CALLBACK_SECRET",
    "EXA_API_KEY",
    "JINA_API_KEY",
    "PYANNOTE_API_KEY",
    "PYANNOTE_API_BASE",
    "DEEPGRAM_API_KEY",
    "CARTESIA_API_KEY",
    "ASSEMBLYAI_API_KEY",
    "SONIOX_API_KEY",
    "FIREWORKS_API_KEY",
    "OPENAI_API_KEY",
    "GLADIA_API_KEY",
    "ELEVENLABS_API_KEY",
    "DASHSCOPE_API_KEY",
    "MISTRAL_API_KEY",
    "AQUAVOICE_API_KEY",
    "COHERE_API_KEY",
}
BILLING = {
    "STRIPE_SECRET_KEY",
    "STRIPE_MONTHLY_PRICE_ID",
    "STRIPE_YEARLY_PRICE_ID",
    "LOOPS_KEY",
}
CORE = BILLING | {"NANGO_API_BASE", "NANGO_API_KEY", "NANGO_WEBHOOK_SIGNING_KEY"}
SYNC = {
    "RESEND_API_KEY",
    "RESEND_FROM_EMAIL",
    "ANARLOG_CLOUDSYNC_DATABASE_ID",
    "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
    "ANARLOG_CLOUDSYNC_PROTOCOL_MODE",
    "ANARLOG_CLOUDSYNC_TOKEN_TTL_SECONDS",
    "ANARLOG_CLOUDSYNC_DESKTOP_TRANSPORT",
    "SQLITECLOUD_CLOUDSYNC_MANAGEMENT_API_KEY",
    "SQLITECLOUD_PROJECT_URL",
    "SQLITECLOUD_TOKEN_ISSUER_API_KEY",
}
ROLES = {"ai": AI, "sync": SYNC, "core": CORE | SYNC, "billing": BILLING}


def select(service, api_secrets, cloudsync_secrets):
    if service not in {*ROLES, "all", "gateway", "legacy"}:
        raise ValueError("Unknown API service")
    allowed = SHARED | ROLES.get(service, AI | CORE | SYNC)
    values = {}
    for source in (api_secrets, cloudsync_secrets):
        for secret in source:
            key, value = secret["key"], secret.get("value", "")
            if key not in allowed:
                continue
            if key in values and values[key] != value:
                raise ValueError(f"Conflicting secret: {key}")
            values[key] = value
    required = {"SUPABASE_URL", "SUPABASE_ANON_KEY", "SUPABASE_SERVICE_ROLE_KEY"}
    if service in {"ai", "all", "gateway", "legacy"}:
        required |= {"OPENROUTER_API_KEY", "API_BASE_URL", "CALLBACK_SECRET"}
        if not any(
            values.get(key)
            for key in AI
            - {
                "OPENROUTER_API_KEY",
                "EXA_API_KEY",
                "JINA_API_KEY",
                "PYANNOTE_API_KEY",
                "API_BASE_URL",
                "CALLBACK_SECRET",
                "PYANNOTE_API_BASE",
            }
        ):
            raise ValueError("At least one transcription provider is required")
    if service in {"billing", "core", "all", "gateway", "legacy"}:
        required |= BILLING
    if service in {"core", "all", "gateway", "legacy"}:
        required |= {"NANGO_API_KEY", "NANGO_WEBHOOK_SIGNING_KEY"}
    if service in {"sync", "core", "all", "gateway", "legacy"}:
        required |= SYNC - {"ANARLOG_CLOUDSYNC_DESKTOP_TRANSPORT"}
    missing = sorted(key for key in required if not values.get(key))
    if missing:
        raise ValueError("Missing required service secrets: " + ", ".join(missing))
    if service == "ai":
        values["API_BASE_URL"] = "https://anarlog-inference.fly.dev"
    return values


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--service", required=True)
    parser.add_argument("--api", required=True)
    parser.add_argument("--cloudsync", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    values = select(
        args.service,
        json.loads(Path(args.api).read_text()),
        json.loads(Path(args.cloudsync).read_text()),
    )
    path = Path(args.output)
    path.touch(mode=0o600)
    path.chmod(0o600)
    with path.open("w") as output:
        for key, value in sorted(values.items()):
            escaped = (
                value.replace("\r\n", "\n").replace("\r", "\n").replace("\n", "\\n")
            )
            output.write(f"{key}={escaped}\n")
    print(f"Prepared {len(values)} runtime secrets for {args.service}")


if __name__ == "__main__":
    main()
