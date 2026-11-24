#!/usr/bin/env bash
set -euo pipefail

# Synchronizes the project screenshots in `assets/` with the copies served
# by the Astro website in `website/public/`. Any screenshot present in
# `assets/` is copied over the matching public file so the site always shows
# the latest capture. Website-only images (logos and icons) are left alone.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
assets_dir="$repo_root/assets"
public_dir="$repo_root/website/public"

if [[ ! -d "$assets_dir" || ! -d "$public_dir" ]]; then
  echo "error: expected '$assets_dir' and '$public_dir' to exist" >&2
  exit 1
fi

changed=0
for source in "$assets_dir"/*.png; do
  [[ -e "$source" ]] || continue
  name="$(basename "$source")"
  target="$public_dir/$name"
  if [[ ! -f "$target" ]] || ! cmp -s "$source" "$target"; then
    cp "$source" "$target"
    printf 'synced %s\n' "$name"
    changed=1
  fi
done

if (( changed == 0 )); then
  echo "website/public is up to date with assets/."
fi
