variable "project_id" {
  type = string
}

variable "region" {
  type    = string
  default = "asia-south1"
}

variable "name_prefix" {
  type    = string
  default = "ros-staging"
}

variable "db_tier" {
  type    = string
  default = "db-custom-1-3840"
}

variable "api_image" {
  type = string
}

variable "database_secret_id" {
  type = string
}

variable "token_pubkey_secret_id" {
  type = string
}
