# ADR 0003: Professional cloud first deployment on Google Cloud

**Status:** Accepted  
**Date:** 18 July 2026  
**Approver:** Gotigin engineering (founder-accountable product default)

## Context

Gotigin evaluated AWS and Google Cloud for startup credits, ongoing cost,
operating simplicity, and Indian deployment requirements. Professional sync and
the owner dashboard need a locked first provider while remaining portable.

## Decision

1. **Architecture remains provider-neutral:**
   `Rust OCI container -> PostgreSQL -> S3-compatible object-store interface`,
   provisioned with OpenTofu/Terraform-compatible modules behind ObjectStore,
   KeyProvider, EmailProvider, and TelemetrySink interfaces.
2. **First deployment target:** Google Cloud in `asia-south1` (Mumbai):
   - Cloud Run for the API
   - Cloud SQL for PostgreSQL (separate migration role and non-owner API role
     without `BYPASSRLS`)
   - Cloud Storage for encrypted baseline/object payloads
3. **Environments:** isolated `development`, `staging`, and `production`
   projects/namespaces with separated secrets.
4. **Diagnostics intake:** `https://diagnostics.ros.gotigin.com/v1/packs`
   (staging may use `https://diagnostics.staging.ros.gotigin.com/v1/packs`);
   retain Owner-consented packs for 90 days unless a shorter legal hold applies.
5. **Data retention (sync events / acknowledgements):** 7 years for financial
   sync facts; operational logs 90 days redacted.
6. Billing owner and GCP organization/account credentials remain founder-held
   secrets and are never committed. Engineering proceeds against local/staging
   Docker Compose and the OpenTofu modules under `infra/`.

## Consequences

- Stage 4 infra and sync client may assume GCP as the first target.
- AWS remains a viable secondary target without application rewrites.
- Local SQLite remains the availability boundary regardless of cloud choice.

## References

- Google Cloud Free: https://cloud.cloud.google.com/free
- Cloud SQL regional availability: https://cloud.google.com/sql/docs/postgres/region-availability-overview
