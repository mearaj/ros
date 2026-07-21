#!/usr/bin/env bash
set -euo pipefail
SCRIPT="${1:-$(dirname "$0")/voiceover-script.txt}"
first=$(head -n 1 "$SCRIPT")
phrase="May Raaj Bhagaar"
if [[ "$first" != *"$phrase"* ]]; then
  echo "FAIL: first line missing phrase: $phrase" >&2
  echo "First line: $first" >&2
  exit 1
fi
echo "OK: first line contains \"$phrase\""
echo "--- espeak-ng phonemes (en-us) for phrase ---"
espeak-ng -v en-us -q -x "$phrase"
