#!/bin/sh
set -eu

arch=$1
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/H5Jive.app"
macos="$app/Contents/MacOS"
resources="$app/Contents/Resources"
mkdir -p "$macos" "$resources" dist
ditto bundle/h5jive "$macos/h5jive"
ditto bundle/lib "$macos/lib"
ditto bundle/settings "$resources/settings"
ditto README.md "$resources/README.md"
ditto LICENSE "$resources/LICENSE"

sips -s format icns website/public/h5jive.png --out "$resources/H5Jive.icns" >/dev/null

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' crates/h5jive/Cargo.toml | head -n 1)
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.h5jive.app</string>
<key>CFBundleName</key><string>H5Jive</string>
<key>CFBundleDisplayName</key><string>H5Jive</string>
<key>CFBundleExecutable</key><string>h5jive</string>
<key>CFBundleIconFile</key><string>H5Jive</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
plutil -lint "$app/Contents/Info.plist"
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict "$app"

image="$stage/image"
mkdir "$image"
ditto "$app" "$image/H5Jive.app"
ln -s /Applications "$image/Applications"
output="dist/h5jive-macos-$arch.dmg"
hdiutil create -volname H5Jive -srcfolder "$image" -ov -format UDZO "$output"
hdiutil verify "$output"

test_home="$stage/test-home"
mkdir "$test_home"
(cd "$stage" && HOME="$test_home" "$app/Contents/MacOS/h5jive" --inspect)
test -f "$test_home/Library/Application Support/H5Jive/settings/DIPfile"
