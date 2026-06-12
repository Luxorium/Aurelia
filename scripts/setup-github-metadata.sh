#!/usr/bin/env bash
set -u

repo="Luxorium/Aurelia"
apply=0

description="Clean-room Minecraft Beta 1.7.3 server rewrite in Rust with a future region-threaded architecture."
homepage="luxorium.dev"
topics=(
  "minecraft"
  "minecraft-server"
  "beta-173"
  "rust"
  "clean-room"
  "protocol"
  "game-server"
  "region-threaded"
  "legacy-minecraft"
  "open-source"
)

usage() {
  cat <<'USAGE'
Usage: scripts/setup-github-metadata.sh [--repo OWNER/REPO] [--apply]

Print recommended GitHub repository metadata commands by default.
Pass --apply to run them with the GitHub CLI.
USAGE
}

quote_command() {
  printf '%q ' "$@"
  printf '\n'
}

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

metadata_cmd=(gh repo edit "$repo" --description "$description" --homepage "$homepage")
topic_cmd=(gh repo edit "$repo")
for topic in "${topics[@]}"; do
  topic_cmd+=(--add-topic "$topic")
done

echo "Recommended GitHub metadata for $repo:"
quote_command "${metadata_cmd[@]}"
quote_command "${topic_cmd[@]}"

if [ "$apply" -ne 1 ]; then
  echo
  echo "Dry run only. Re-run with --apply to execute these commands."
  exit 0
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "error: GitHub CLI 'gh' is not installed or not on PATH." >&2
  echo "Install gh and authenticate with 'gh auth login', then re-run with --apply." >&2
  exit 1
fi

echo
echo "Applying repository metadata..."
"${metadata_cmd[@]}"
"${topic_cmd[@]}"
echo "Done."
