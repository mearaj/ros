#!/usr/bin/env bash
# Creates a release-candidate SBOM *placeholder* that records provenance
# requirements. It is not a CycloneDX/SPDX publication SBOM until a reviewed
# generator and license notice review are wired for the exact release artifact.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <output-directory>" >&2
  exit 2
fi

output_dir=$1
mkdir -p "$output_dir"
revision=$(git rev-parse HEAD 2>/dev/null || echo "uncommitted")
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat >"$output_dir/SBOM-PLACEHOLDER.md" <<EOF
# Artifact SBOM placeholder

- generated_at_utc: ${timestamp}
- source_revision: ${revision}
- status: placeholder — not a publication SBOM

## Required before public release

1. Generate an artifact-specific CycloneDX or SPDX SBOM for each signed
   desktop/API image.
2. Attach license/notice review and vulnerability disposition.
3. Sign the SBOM and store it beside the release checksums.
4. Retain the baseline dependency evidence from
   \`generate-dependency-evidence.sh\` as the lockfile inventory input.

See docs/runbooks/dependency-evidence-and-sbom.md.
EOF

(
  cd "$output_dir"
  sha256sum SBOM-PLACEHOLDER.md > SHA256SUMS
)

echo "Wrote $output_dir/SBOM-PLACEHOLDER.md"
