variable "name" { type = string }
variable "region" { type = string }
variable "image" { type = string }
variable "database_secret_id" { type = string }
variable "token_pubkey_secret_id" { type = string }
variable "deployment_environment" { type = string }

resource "google_cloud_run_v2_service" "api" {
  name     = var.name
  location = var.region

  template {
    containers {
      image = var.image
      env {
        name  = "ROS_DEPLOYMENT_ENVIRONMENT"
        value = var.deployment_environment
      }
      env {
        name = "ROS_DATABASE_URL"
        value_source {
          secret_key_ref {
            secret  = var.database_secret_id
            version = "latest"
          }
        }
      }
      env {
        name = "ROS_SYNC_TOKEN_PUBLIC_KEY_FILE"
        value_source {
          secret_key_ref {
            secret  = var.token_pubkey_secret_id
            version = "latest"
          }
        }
      }
    }
  }
}

output "uri" {
  value = google_cloud_run_v2_service.api.uri
}
