#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

if command -v gitleaks >/dev/null 2>&1; then
  gitleaks detect --no-banner --source .
  exit
fi

if rg -n --hidden -g '!target/**' -g '!.git/**' \
  '(BEGIN (RSA|OPENSSH|EC) PRIVATE KEY|AKIA[0-9A-Z]{16}|github_pat_[A-Za-z0-9_]{20,})' .; then
  echo "Potential secret material found." >&2
  exit 1
fi
echo "No obvious secrets found (install gitleaks for the full scan)."
