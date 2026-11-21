#!/bin/sh
# Assemble a local, ad-hoc signed NSH app with the same layout as the release.
set -eu
cd "$(dirname "$0")/.."
case "${1:-release}" in
  release) profile=release; set -- --release ;;
  debug) profile=debug; set -- ;;
  *) echo "Usage: $0 [debug|release]" >&2; exit 2 ;;
esac
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --locked -p biblio-desktop "$@"
target_dir=$(cargo metadata --no-deps --format-version 1 | /usr/bin/plutil -extract target_directory raw -o - -)
contents="$target_dir/$profile/Biblio.app/Contents"
mkdir -p "$contents/MacOS" "$contents/Resources"
install -m 755 "$target_dir/$profile/biblio-desktop" "$contents/MacOS/biblio-desktop"
install -m 644 packaging/Info.plist "$contents/Info.plist"
install -m 644 packaging/icons/icon.icns "$contents/Resources/AppIcon.icns"
codesign --force --sign - "$contents/.."
echo "$target_dir/$profile/Biblio.app"
