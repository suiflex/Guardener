#!/usr/bin/env bash
# Brings the stub workflows already installed across the organization back in
# line with templates/workflows/.
#
# This exists because `hygiene --fix` only ever adds. That rule is deliberate and
# not up for negotiation — a bot that quietly revises decisions across an
# organization is worse than the drift it was meant to catch. But it leaves a
# real gap: once a stub is installed, no change to the template ever reaches it.
# Adding a secret to templates/workflows/review.yml is the case that found this.
#
# So the edit happens here instead, and the difference that matters is who runs
# it. Nothing schedules this. It is a person, at a keyboard, who has read the
# diff it prints first — the same bargain as `hygiene --fix` living behind a
# workflow_dispatch rather than the daily cron.
#
#   ./scripts/sync-stubs.sh                            # drift, every template
#   ./scripts/sync-stubs.sh --apply review.yml         # replace that one file
#   ./scripts/sync-stubs.sh --apply review.yml websift # in that one repository
#
# Reporting covers every template, because reading is free and seeing the whole
# picture is the point. Writing does not: `--apply` demands one template by
# name, and will not take "all of them".
#
# That asymmetry is not caution for its own sake. The first run of this script
# found suiflex.yml in ForgeGuard carrying a second job this template has never
# had — a labeler, with a comment explaining the ordering race it was written to
# fix. A blanket sync would have deleted it and the reason for it. A stub is a
# starting point, and repositories are allowed to have grown past theirs.
#
# Needs `gh` authenticated with permission to write .github/workflows/ in each
# repository, which is a scope a personal token does not carry by default.

set -euo pipefail

cd "$(dirname "$0")/.."

REGISTRY=config/repositories.toml
TEMPLATES=templates/workflows
# Guardener holds the reusable workflows these stubs call, at the very paths a
# stub would occupy. Syncing a template over one would replace the workflow with
# a caller of itself.
SELF=suiflex/Guardener

apply=""
only=""
if [ "${1:-}" = "--apply" ]; then
  apply=1
  shift
  only="${1:-}"
  if [ -z "$only" ] || [ ! -f "$TEMPLATES/$only" ]; then
    echo "usage: $0 --apply <template> [repo...]" >&2
    echo "Name exactly one file from $TEMPLATES/ — this overwrites what is" >&2
    echo "installed, and a repository may have grown past its stub:" >&2
    ls "$TEMPLATES" | sed 's/^/  /' >&2
    exit 2
  fi
  shift
fi

# One repository per `name = "owner/repo"` line, in registry order. Read with a
# loop rather than `mapfile`, which macOS's bash 3.2 does not have.
repos=()
while IFS= read -r line; do
  repos+=("$line")
done < <(sed -n 's/^name = "\(.*\)"$/\1/p' "$REGISTRY")
[ $# -gt 0 ] && repos=("$@")

# One template against one repository. Returns 1 when they differ, so the caller
# can tell at the end whether anything is out of step.
#
# Split out rather than nested in the loop below because the two are separate
# questions — which repositories to walk, against whether one file in one of them
# matches — and reading it as a single doubly-nested block meant holding both.
check_one() {
  local repo=$1 branch=$2 template=$3
  local name path meta sha installed
  name=$(basename "$template")
  path=".github/workflows/$name"

  # Report on everything; write only to the one file named on the command line,
  # so nothing is overwritten as a side effect of looking.
  if [ -n "$apply" ] && [ "$name" != "$only" ]; then
    return 0
  fi

  if ! meta=$(gh api "/repos/$repo/contents/$path?ref=$branch" 2>/dev/null); then
    # Absent is hygiene's job, not this script's: `--fix` adds, and adding is
    # the one thing it is allowed to do.
    printf '%-24s %-34s absent — `hygiene --fix` adds it\n' "$repo" "$path"
    return 0
  fi
  sha=$(jq -r .sha <<<"$meta")
  installed=$(jq -r .content <<<"$meta" | base64 --decode)

  if [ "$installed" = "$(cat "$template")" ]; then
    printf '%-24s %-34s in step\n' "$repo" "$path"
    return 0
  fi

  printf '%-24s %-34s DRIFTED on %s\n' "$repo" "$path" "$branch"
  diff -u --label "installed" <(printf '%s' "$installed") \
          --label "template"  "$template" || true

  if [ -n "$apply" ]; then
    gh api -X PUT "/repos/$repo/contents/$path" \
      -f message="ci: sync $name with the organization template" \
      -f branch="$branch" \
      -f sha="$sha" \
      -f content="$(base64 < "$template" | tr -d '\n')" \
      --jq '"  committed " + .commit.sha'
  fi
  return 1
}

# Every template against one repository. Returns 1 if any of them differ.
check_repo() {
  local repo=$1 branch=$2
  local template rc=0
  for template in "$TEMPLATES"/*.yml; do
    check_one "$repo" "$branch" "$template" || rc=1
  done
  return $rc
}

drift=0
for repo in "${repos[@]}"; do
  case "$repo" in */*) ;; *) repo="suiflex/$repo" ;; esac
  if [ "$repo" = "$SELF" ]; then
    printf '%-24s skipped — it hosts the workflows these stubs call\n' "$repo"
    continue
  fi

  branch=$(gh api "/repos/$repo" --jq .default_branch)
  check_repo "$repo" "$branch" || drift=1
done

if [ -z "$apply" ] && [ "$drift" = 1 ]; then
  echo
  echo "Nothing was written. Re-run with --apply once the diffs above look right."
fi
