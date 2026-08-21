# Self-hosted and certified-cloud deployment

ANLG-137.

## v1 recommended story

Ship a **customer-controlled data plane**: commercially licensed control plane + Postgres + object storage + Meet worker + optional Zoom RTMS, running in the customer's VPC. The MIT desktop/web client still runs on Mac and Windows endpoints. This is not AGPL and not an air-gapped meeting joiner.

Eval: `enterprise/control-plane/compose.yaml`. Prod: `enterprise/deploy/compose.prod.yaml` and Helm chart `anarlog-capture`. Operations: `enterprise/deploy/docs/operations.md`.

## Later SKUs

- Certified cloud: Fastrepl operates the same images in a named region under a BAA/DPA
- Full self-host of sync/auth (Supabase) is a separate product; v1 still uses Fastrepl or customer Supabase for identity
- Windows vs Mac local data paths stay in the MIT client; capture workers are Linux containers except the Teams Graph sidecar (Windows Server)

## Tradeoffs

Customers who need "no Fastrepl in the data path for meetings" take the Helm chart. Customers who need "no public meeting egress" cannot capture Meet/Zoom/Teams. Customers who need Teams take Azure + Windows Server after the Meet reliability gate (ANLG-232).
