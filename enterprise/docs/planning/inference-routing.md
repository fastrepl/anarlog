# Enterprise-controlled inference routing

ANLG-132.

## Modes

Transcription:

- Local machine (on-device STT already in the desktop app)
- Customer-hosted STT (`ANARLOG_ENTERPRISE_STT_URL` on capture workers)
- Approved third-party cloud (BYO key in settings)
- Certified-cloud provider (later Fastrepl-operated region)

LLM:

- Local model
- Customer-hosted gateway
- Approved external API (BYO key)
- Certified-cloud provider (later)

## Policy

Workspace policy `model_training_opt_out` defaults on for enterprise. Admins can disable external model calls by leaving only local/customer-hosted providers in the allowed set. The desktop AI settings surface names the destination ("this computer", "your gateway", "approved cloud") and never the plumbing.

v1 ships the policy flag and customer STT URL. Enforced provider allowlists in the LLM/STT proxies are the next implementation slice.
