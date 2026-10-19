#!/bin/bash
# build.sh <rom>... [--keys aes_keys.txt] [--out dir]
#
# Builds Pocket Native and games for Android handhelds (the Anbernic RG DS
# under GammaOS Next first) from your own ROMs, decrypted .3ds/.cci files or
# .cia files with your console's aes_keys.txt, inside a container (podman or
# docker). Leaves "Pocket Native Android" in the output directory, build/ by
# default, which scripts/android/probe.sh pushes to the handheld with adb.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
roms=() keys="" out="$repo/build"
while [ $# -gt 0 ]; do
    case "$1" in
        --keys) keys="$(realpath "$2")"; shift 2 ;;
        --out) out="$(realpath -m "$2")"; shift 2 ;;
        -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
        *) roms+=("$(realpath -m "$1")"); shift ;;
    esac
done
if [ ${#roms[@]} -eq 0 ]; then
    echo "usage: scripts/android/build.sh <rom>... [--keys aes_keys.txt] [--out dir]"
    exit 2
fi
for rom in "${roms[@]}"; do
    [ -f "$rom" ] || { echo "no such file: $rom"; exit 2; }
done
engine=$(command -v podman || command -v docker) || { echo "the build needs podman or docker"; exit 1; }
image=pocket-native-android:$(sha256sum "$repo/scripts/android/Containerfile" | cut -c1-12)
if ! "$engine" image inspect "$image" >/dev/null 2>&1; then
    echo "== making the Android build container, once (the NDK is about 1 GB)"
    "$engine" build -f "$repo/scripts/android/Containerfile" -t "$image" "$repo/scripts/android"
fi

# a work folder of its own, so the Android build's crates and objects never
# mix with the ArkOS build's
mkdir -p "$out/.work-android"
user=(--user "$(id -u):$(id -g)")
[ "$(basename "$engine")" = podman ] && user=(--userns=keep-id)
failed=()
for rom in "${roms[@]}"; do
    mounts=(-v "$repo:/src:ro" -v "$out/.work-android:/work" -v "$out:/out" -v "$(dirname "$rom"):/rom:ro")
    args=("/rom/$(basename "$rom")")
    rom_keys="$keys"
    if [ -z "$rom_keys" ] && [[ "${rom,,}" == *.cia ]]; then
        for near in "$(dirname "$rom")/aes_keys.txt" "$repo/aes_keys.txt"; do
            [ -f "$near" ] && { rom_keys="$near"; break; }
        done
    fi
    if [ -n "$rom_keys" ]; then
        mounts+=(-v "$(dirname "$rom_keys"):/keys:ro")
        args+=("/keys/$(basename "$rom_keys")")
    fi
    "$engine" run --rm "${user[@]}" "${mounts[@]}" -e CARGO_HOME=/work/cargo -e CARGO_TARGET_DIR=/work/target -e HOME=/work \
        "$image" bash /src/scripts/android/inside.sh "${args[@]}" || failed+=("$rom")
done
if [ ${#failed[@]} -gt 0 ]; then
    printf '== these did not build:\n'
    printf '   %s\n' "${failed[@]}"
    exit 1
fi
