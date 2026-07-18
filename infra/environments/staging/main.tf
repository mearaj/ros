terraform {
  required_version = ">= 1.6.0"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.40"
    }
  }
}

provider "google" {
  project = var.project_id
  region  = var.region
}

module "database" {
  source              = "../../modules/cloud_sql"
  name                = "${var.name_prefix}-postgres"
  region              = var.region
  database_version    = "POSTGRES_16"
  tier                = var.db_tier
  api_runtime_user    = "ros_api"
  migration_user      = "ros_migrator"
}

module "object_store" {
  source = "../../modules/object_store"
  name   = "${var.name_prefix}-baselines"
  region = var.region
}

module "api" {
  source                = "../../modules/cloud_run_api"
  name                  = "${var.name_prefix}-api"
  region                = var.region
  image                 = var.api_image
  database_secret_id    = var.database_secret_id
  token_pubkey_secret_id = var.token_pubkey_secret_id
  deployment_environment = "staging"
}
