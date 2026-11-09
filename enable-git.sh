#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
if [[ ! -d .spacegit/.git ]]; then
  echo "H5Jive repository metadata is missing: .spacegit/.git" >&2
  exit 1
fi
if [[ -e .git || -L .git ]]; then
  echo "A .git entry already exists."
else
  ln -s .spacegit/.git .git
  echo "Created .git link to the H5Jive repository."
fi
git status --short
