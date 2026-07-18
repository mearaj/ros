variable "name" { type = string }
variable "region" { type = string }
variable "database_version" { type = string }
variable "tier" { type = string }
variable "api_runtime_user" { type = string }
variable "migration_user" { type = string }

resource "google_sql_database_instance" "primary" {
  name             = var.name
  region           = var.region
  database_version = var.database_version

  settings {
    tier = var.tier
    ip_configuration {
      ipv4_enabled = false
    }
    database_flags {
      name  = "cloudsql.iam_authentication"
      value = "on"
    }
  }
}

resource "google_sql_database" "ros" {
  name     = "restaurant_os"
  instance = google_sql_database_instance.primary.name
}

output "connection_name" {
  value = google_sql_database_instance.primary.connection_name
}

output "api_runtime_user" {
  value = var.api_runtime_user
}

output "migration_user" {
  value = var.migration_user
}
