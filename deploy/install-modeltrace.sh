#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="$repo_root/.runtime/data/modeltrace"
repository="https://github.com/xqy2006/ModelTrace.git"
revision="55a2e4a55170423b484d701e9a82ab62b268c811"

cd "$repo_root"
if [[ -d "$target/.git" ]]; then
  tracked_changes=$(git -C "$target" status --porcelain --untracked-files=no)
  other_untracked=$(git -C "$target" ls-files --others --exclude-standard | awk '$0 !~ /^\.venv(\/|$)/')
  if [[ -n "$tracked_changes" || -n "$other_untracked" ]]; then
    printf 'ModelTrace checkout has local changes: %s\n' "$target" >&2
    exit 1
  fi
else
  if [[ -e "$target" ]]; then
    printf 'Refusing to overwrite existing path: %s\n' "$target" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$target")"
  git clone --filter=blob:none --no-checkout "$repository" "$target"
fi

git -C "$target" fetch --depth 1 origin "$revision"
git -C "$target" checkout --detach FETCH_HEAD
if [[ "$(git -C "$target" rev-parse HEAD)" != "$revision" ]]; then
  printf 'ModelTrace revision did not match the pinned commit.\n' >&2
  exit 1
fi
printf '%s\n' "$revision" > "$target/.git/CPR_REVISION"
test -s "$target/data/unified_bank.json"
test -s "$target/challenge_suite.py"

if [[ -f "$repo_root/deploy/compose.yaml" && -f "$repo_root/deploy/config.yaml" ]]; then
  docker compose -f deploy/compose.yaml run --rm --no-deps --user 0:0 \
    --entrypoint /bin/sh codex-proxy-rs -ec '
      python3 -m venv /app/.runtime/data/modeltrace/.venv
      PYTHONDONTWRITEBYTECODE=1 /app/.runtime/data/modeltrace/.venv/bin/python -m pip install --disable-pip-version-check "numpy>=1.26,<3"
      PYTHONDONTWRITEBYTECODE=1 /app/.runtime/data/modeltrace/.venv/bin/python -c "from challenge_suite import fingerprint_suite; from fingerprint import load_bank; assert len(fingerprint_suite()) >= 3; assert load_bank()"
    '
else
  python3 -m venv "$target/.venv"
  (
    cd "$target"
    PYTHONDONTWRITEBYTECODE=1 "$target/.venv/bin/python" -m pip install --disable-pip-version-check "numpy>=1.26,<3"
    PYTHONDONTWRITEBYTECODE=1 "$target/.venv/bin/python" -c 'from challenge_suite import fingerprint_suite; from fingerprint import load_bank; assert len(fingerprint_suite()) >= 3; assert load_bank()'
  )
fi

printf 'ModelTrace installed at %s (%s)\n' "$target" "$revision"
