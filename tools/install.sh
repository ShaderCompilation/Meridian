#!/usr/bin/env bash
# Build Meridian in release mode and install it for the current user.
#   tools/install.sh            install
#   tools/install.sh uninstall  remove (your database is kept)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
apps_dir="$data_dir/applications"
icons_dir="$data_dir/icons/hicolor"

refresh() {
    update-desktop-database -q "$apps_dir" 2>/dev/null || true
    gtk-update-icon-cache -q -t -f "$icons_dir" 2>/dev/null || true
}

if [[ "${1:-}" == "uninstall" ]]; then
    rm -fv "$bin_dir/meridian" \
        "$apps_dir/meridian.desktop" \
        "$icons_dir/scalable/apps/meridian.svg" \
        "$icons_dir"/*/apps/meridian.png
    refresh
    echo "Uninstalled. Data left in $data_dir/meridian"
    exit 0
fi

cargo build --release --manifest-path "$root/Cargo.toml"

install -Dm755 "$root/target/release/meridian" "$bin_dir/meridian"
install -Dm644 "$root/assets/meridian.desktop" "$apps_dir/meridian.desktop"
install -Dm644 "$root/assets/icons/meridian.svg" "$icons_dir/scalable/apps/meridian.svg"

# PNG fallbacks for apps that don't render SVG icons.
if command -v magick >/dev/null; then
    for size in 32 48 64 128 256 512; do
        mkdir -p "$icons_dir/${size}x${size}/apps"
        magick -background none "$root/assets/icons/meridian.svg" -resize "${size}x${size}" \
            "$icons_dir/${size}x${size}/apps/meridian.png"
    done
fi

refresh
echo "Installed $bin_dir/meridian"
