#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
report="$repo_dir/settings/settings-report.pdf"
cheatsheet="$repo_dir/assets/dipl-cheatsheet.pdf"

if ! command -v snt >/dev/null 2>&1; then
    echo 'Skipping settings PDF: snt is not on PATH.' >&2
    exit 0
fi
if ! command -v pdfjam >/dev/null 2>&1; then
    echo 'Skipping settings PDF: pdfjam is not on PATH.' >&2
    exit 0
fi
if ! command -v pdflatex >/dev/null 2>&1; then
    echo 'Skipping settings PDF: pdflatex is not on PATH.' >&2
    exit 0
fi

# The report is a source reference. Local overrides must not appear in a
# generated file that can be committed or distributed.
if [ -f "$report" ] && [ "$report" -nt "$cheatsheet" ] && [ "$report" -nt "$repo_dir/generate-settings-report.sh" ] &&
    ! find "$repo_dir/settings/build" "$repo_dir/settings/runtime" -type f \( -name '*.dip' -o -name DIPfile \) -newer "$report" -print -quit | grep -q .; then
    exit 0
fi

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/h5jive-settings-report.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT HUP INT TERM
cp -R "$repo_dir/settings/runtime" "$work_dir/runtime"
cp -R "$repo_dir/settings/build" "$work_dir/build"
: > "$work_dir/runtime/overrides.dip"
cd "$work_dir"

snt report --project runtime/DIPfile --format pdf \
    --output runtime.pdf --title 'H5Jive runtime settings' --author H5Jive
snt report --project build/DIPfile --format pdf \
    --output build.pdf --title 'H5Jive build settings' --author H5Jive
pdfjam --quiet runtime.pdf build.pdf "$cheatsheet" \
    --outfile settings-report.pdf

cp "$work_dir/settings-report.pdf" "$repo_dir/settings/.settings-report.pdf.tmp"
mv "$repo_dir/settings/.settings-report.pdf.tmp" "$report"
echo "Generated $report"
