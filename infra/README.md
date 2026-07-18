# Infrastructure (OpenTofu / Terraform compatible)

Provider-neutral modules targeting the first Google Cloud deployment in
`asia-south1` (ADR 0003).

## Layout

```text
infra/
  environments/
    staging/
      main.tf
      variables.tf
      terraform.tfvars.example
  modules/
    cloud_sql/
    cloud_run_api/
    object_store/
```

## Apply order

1. Create GCP projects and billing linkage (founder credential step).
2. `cd infra/environments/staging`
3. Copy `terraform.tfvars.example` → `terraform.tfvars` (never commit secrets).
4. `tofu init && tofu plan && tofu apply`
5. Apply cloud migrations with the migration role (not the API runtime role).
6. Deploy the API image to Cloud Run with secret-manager bindings.

## Database roles

- `ros_migrator` — owns schema changes; not used by the API at runtime.
- `ros_api` — least-privilege runtime role **without** `BYPASSRLS`.

## Secrets

Store database URLs, token public keys, and diagnostics intake credentials in
Secret Manager. Local development uses `.env` files that are gitignored; see
`services/api/.env.example`.
