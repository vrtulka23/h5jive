#!/bin/sh
set -eu

appdir="$RUNNER_TEMP/H5Jive.AppDir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/h5jive" dist
cp bundle/h5jive "$appdir/usr/bin/h5jive"
cp -R bundle/lib "$appdir/usr/bin/lib"
cp -R bundle/settings "$appdir/usr/share/h5jive/settings"
cp website/public/h5jive.png "$appdir/h5jive.png"
cat > "$appdir/h5jive.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=H5Jive
Comment=Explore scientific HDF5 and NetCDF data
Exec=h5jive
Icon=h5jive
Categories=Science;Education;
Terminal=false
EOF
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
set -eu
appdir=${APPDIR:-$(CDPATH= cd "$(dirname "$0")" && pwd)}
exec "$appdir/usr/bin/h5jive" "$@"
EOF
chmod +x "$appdir/AppRun" "$appdir/usr/bin/h5jive"

tool="$RUNNER_TEMP/appimagetool-x86_64.AppImage"
curl -fsSL https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage -o "$tool"
printf '%s  %s\n' ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0 "$tool" | sha256sum -c -
chmod +x "$tool"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" "$appdir" dist/h5jive-linux-x86_64.AppImage
chmod +x dist/h5jive-linux-x86_64.AppImage

test_data=$(mktemp -d)
trap 'rm -rf "$test_data"' EXIT
XDG_DATA_HOME="$test_data" APPIMAGE_EXTRACT_AND_RUN=1 dist/h5jive-linux-x86_64.AppImage --inspect
test -f "$test_data/h5jive/settings/DIPfile"
