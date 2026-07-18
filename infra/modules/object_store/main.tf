variable "name" { type = string }
variable "region" { type = string }

resource "google_storage_bucket" "baselines" {
  name                        = var.name
  location                    = var.region
  uniform_bucket_level_access = true
  versioning {
    enabled = true
  }
}

output "bucket_name" {
  value = google_storage_bucket.baselines.name
}
