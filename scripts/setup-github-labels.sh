#!/usr/bin/env bash
set -u

repo="Luxorium/Aurelia"
apply=0

usage() {
  cat <<'USAGE'
Usage: scripts/setup-github-labels.sh [--repo OWNER/REPO] [--apply]

Print recommended GitHub labels by default.
Pass --apply to create or update labels with the GitHub CLI.
USAGE
}

labels=(
  "good first issue|7057ff|Approachable issue for new contributors"
  "help wanted|008672|Extra contributor attention is welcome"
  "documentation|0075ca|Documentation, README, or project presentation"
  "clean-room|5319e7|Clean-room research, provenance, or evidence"
  "protocol|1d76db|Beta 1.7.3 protocol work"
  "compatibility|0e8a16|Real-client compatibility behavior"
  "world|c2e0c6|World storage, chunks, blocks, or terrain"
  "inventory|fbca04|Inventory, windows, slots, or item stacks"
  "entities|d93f0b|Entities, mobs, item entities, or combat"
  "region-threading|5319e7|Region ownership, scheduling, or threading"
  "bug|d73a4a|Something is not working"
  "enhancement|a2eeef|New feature or improvement"
  "question|d876e3|Question or discussion item"
  "needs-trace|f9d0c4|Needs clean packet trace or real-client evidence"
  "legal-clean-room|b60205|Legal safety or clean-room policy concern"
)

while [ "$#" -gt 0 ]; do
  case "$1" in
    --apply)
      apply=1
      ;;
    --repo)
      shift
      if [ "$#" -eq 0 ]; then
        echo "error: --repo requires OWNER/REPO" >&2
        exit 2
      fi
      repo="$1"
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

echo "Recommended labels for $repo:"
for label in "${labels[@]}"; do
  IFS='|' read -r name color description <<< "$label"
  printf '  - %s (#%s): %s\n' "$name" "$color" "$description"
done

if [ "$apply" -ne 1 ]; then
  echo
  echo "Dry run only. Re-run with --apply to create or update these labels."
  exit 0
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "error: GitHub CLI 'gh' is not installed or not on PATH." >&2
  echo "Install gh and authenticate with 'gh auth login', then re-run with --apply." >&2
  exit 1
fi

echo
echo "Creating or updating labels..."
for label in "${labels[@]}"; do
  IFS='|' read -r name color description <<< "$label"
  if gh label create "$name" --repo "$repo" --color "$color" --description "$description"; then
    echo "created: $name"
  else
    echo "label may already exist, attempting update: $name"
    if gh label edit "$name" --repo "$repo" --color "$color" --description "$description"; then
      echo "updated: $name"
    else
      echo "warning: could not create or update label: $name" >&2
    fi
  fi
done
echo "Done."
