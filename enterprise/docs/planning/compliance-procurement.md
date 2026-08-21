# Compliance and procurement roadmap

ANLG-136.

## Immediate (sales/procurement, no certification claimed)

- DPA, privacy policy, ToS, subprocessor list
- Security questionnaire answers grounded in: local-first E2EE, customer-hosted capture option, metadata-only analytics, offline license, no mandatory telemetry
- Incident response and access-review procedures for Fastrepl-operated cloud
- Data residency: customer-hosted data plane in the customer's region; Fastrepl cloud is US unless a certified-cloud SKU exists

## Certification programs (later, separately funded)

1. SOC 2 Type I then Type II for Fastrepl-operated cloud and control plane
2. ISO 27001 after SOC 2 evidence exists
3. AIUC-1 for agent access once CLI/MCP permission boundaries are productized (ANLG-138)
4. HIPAA: only if we take on BAAs for certified-cloud; self-hosted customers own their covered-entity posture

Do not claim "HIPAA compliant" or "SOC 2" in product copy until the named program is complete.
