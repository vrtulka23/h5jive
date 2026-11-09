#!/bin/sh
# Install or uninstall a native release in the current user's home directory.
set -eu

usage() {
    cat <<'EOF'
Usage: sh install.sh [--version TAG] [--uninstall]

Installs the latest release by default. No administrator access is needed.
Set H5JIVE_INSTALL_DIR and H5JIVE_BIN_DIR to change the destinations.
--uninstall removes the program and launcher, retaining the local settings directory.
EOF
}

version=latest
uninstall=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --uninstall)
            uninstall=true
            shift
            ;;
        --version)
            [ "$#" -ge 2 ] || { usage >&2; exit 2; }
            version=$2
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done
case "$version" in
    ''|*[!A-Za-z0-9._-]*) echo "Invalid release tag: $version" >&2; exit 2 ;;
esac

install_dir=${H5JIVE_INSTALL_DIR:-"$HOME/.local/share/h5jive"}
bin_dir=${H5JIVE_BIN_DIR:-"$HOME/.local/bin"}
case "$install_dir:$bin_dir" in
    /*:/*) ;;
    *) echo 'Installation and bin directories must be absolute paths.' >&2; exit 2 ;;
esac
case "$install_dir" in
    /|*/../*|*/..|*/./*|*/.)
        echo 'Choose a non-root installation directory without dot components.' >&2
        exit 2
        ;;
esac
if [ -e "$install_dir" ] && [ ! -d "$install_dir" ]; then
    echo "Installation path is not a directory: $install_dir" >&2
    exit 1
fi
if [ -L "$install_dir" ]; then
    echo "Installation path must not be a symbolic link: $install_dir" >&2
    exit 1
fi
link="$bin_dir/h5jive"
is_our_launcher() {
    if [ -L "$link" ]; then
        [ "$(readlink "$link")" = "$install_dir/h5jive" ]
    elif [ -f "$link" ]; then
        grep -q '^# H5Jive launcher$' "$link" &&
            grep -Fq "$install_dir/h5jive" "$link"
    else
        return 1
    fi
}
if [ "$uninstall" = true ]; then
    if [ ! -d "$install_dir" ] || [ ! -e "$install_dir/h5jive" ]; then
        if is_our_launcher; then rm -f "$link"; fi
        echo 'H5Jive is not installed at the selected location.'
        exit 0
    fi
    if [ ! -f "$install_dir/settings/DIPfile" ]; then
        echo "Refusing to remove an unrecognized installation: $install_dir" >&2
        exit 1
    fi
    if is_our_launcher; then rm -f "$link"; fi
    rm -f "$install_dir/h5jive" "$install_dir/README.md" "$install_dir/LICENSE"
    if [ -d "$install_dir/lib" ]; then rm -rf "$install_dir/lib"; fi
    echo "Uninstalled H5Jive from $install_dir"
    echo "Settings retained at $install_dir/settings"
    exit 0
fi
if [ -e "$link" ] || [ -L "$link" ]; then
    if ! is_our_launcher; then
        echo "Existing file would be replaced: $link" >&2
        exit 1
    fi
fi

case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) archive=h5jive-linux-x86_64.tar.gz ;;
    Darwin:x86_64) archive=h5jive-macos-x86_64.tar.gz ;;
    Darwin:arm64|Darwin:aarch64) archive=h5jive-macos-aarch64.tar.gz ;;
    *) echo 'Unsupported platform; use the Windows PowerShell installer on Windows.' >&2; exit 1 ;;
esac

command -v curl >/dev/null 2>&1 || { echo 'curl is required.' >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo 'tar is required.' >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
    hash_command=sha256sum
elif command -v shasum >/dev/null 2>&1; then
    hash_command=shasum
else
    echo 'sha256sum or shasum is required.' >&2
    exit 1
fi

repository=${H5JIVE_REPOSITORY:-vrtulka23/h5jive}
case "$repository" in
    */*) ;;
    *) echo 'H5JIVE_REPOSITORY must be owner/repository.' >&2; exit 2 ;;
esac
if [ -n "${H5JIVE_DOWNLOAD_BASE:-}" ]; then
    base=$H5JIVE_DOWNLOAD_BASE
elif [ "$version" = latest ]; then
    base="https://github.com/$repository/releases/latest/download"
else
    base="https://github.com/$repository/releases/download/$version"
fi

parent=$(dirname "$install_dir")
mkdir -p "$parent" "$bin_dir"
stage=$(mktemp -d "$parent/.h5jive-install.XXXXXX")
trap 'rm -rf "$stage"' 0
trap 'exit 1' 1 2 3 15

echo "Downloading $archive"
curl -fsSL "$base/$archive" -o "$stage/$archive"
curl -fsSL "$base/SHA256SUMS" -o "$stage/SHA256SUMS"
expected=$(awk -v name="$archive" '$2 == name { print $1 }' "$stage/SHA256SUMS")
if [ "${#expected}" -ne 64 ]; then
    echo "Missing checksum for $archive" >&2
    exit 1
fi
if [ "$hash_command" = shasum ]; then
    actual=$(shasum -a 256 "$stage/$archive" | awk '{ print $1 }')
else
    actual=$(sha256sum "$stage/$archive" | awk '{ print $1 }')
fi
if [ "$actual" != "$expected" ]; then
    echo "Checksum mismatch for $archive" >&2
    exit 1
fi

package="$stage/package"
mkdir "$package"
tar -xzf "$stage/$archive" -C "$package"
if [ ! -x "$package/h5jive" ] || [ ! -f "$package/settings/DIPfile" ]; then
    echo 'Release archive is missing the executable or DIPL settings.' >&2
    exit 1
fi
if [ -f "$install_dir/settings/overrides.dip" ]; then
    # Local repository paths belong to the user, so carry them into upgrades.
    cp "$install_dir/settings/overrides.dip" "$package/settings/overrides.dip"
fi
if [ -d "$install_dir/settings/.cache" ]; then
    # Product and overpass indexes are expensive to regenerate on large repositories.
    cp -R "$install_dir/settings/.cache" "$package/settings/.cache"
fi
(cd "$stage" && "$package/h5jive" --inspect)

backup="$stage/previous"
if [ -d "$install_dir" ]; then
    mv "$install_dir" "$backup"
fi
if ! mv "$package" "$install_dir"; then
    if [ -d "$backup" ]; then mv "$backup" "$install_dir"; fi
    echo 'Could not install the downloaded release.' >&2
    exit 1
fi
quoted_target=$(printf '%s' "$install_dir/h5jive" | sed "s/'/'\\\\''/g")
cat > "$stage/launcher" <<EOF
#!/bin/sh
# H5Jive launcher
exec '$quoted_target' "\$@"
EOF
chmod +x "$stage/launcher"
mv -f "$stage/launcher" "$link"

echo "Installed H5Jive at $install_dir"
echo "Executable: $link"
echo "Dataset paths: $install_dir/settings/overrides.dip"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "Add $bin_dir to PATH, or run $link directly." ;;
esac
