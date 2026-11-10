#!/usr/bin/env bash
# Build and install Meridian for the current user. Source checkouts are always
# rebuilt so running this script updates an existing install. Release archives
# use the prebuilt binary next to this script's parent folder.
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

action="${1:-install}"
if [[ "$action" == "uninstall" ]]; then
    rm -fv "$bin_dir/meridian" \
        "$apps_dir/meridian.desktop" \
        "$icons_dir/scalable/apps/meridian.svg" \
        "$icons_dir"/*/apps/meridian.png
    refresh
    echo "Uninstalled. Data left in $data_dir/meridian"
    exit 0
fi

if [[ "$action" != "install" || $# -gt 1 ]]; then
    echo "Usage: $0 [install|uninstall]" >&2
    exit 2
fi

if [[ -f "$root/Cargo.toml" ]]; then
    command -v cargo >/dev/null || {
        echo "Rust/Cargo is required to build Meridian. Install Rust 1.90 or newer." >&2
        exit 1
    }
    cargo build --release --manifest-path "$root/Cargo.toml"
    bin="$root/target/release/meridian"
elif [[ -x "$root/meridian" ]]; then
    bin="$root/meridian"
else
    echo "No Cargo.toml or prebuilt meridian binary found in $root" >&2
    exit 1
fi

install -Dm755 "$bin" "$bin_dir/meridian"
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
