#!/bin/bash
# inside.sh <rom> [aes_keys.txt]: the Android build, in the container. /src
# is the repository, read only, /work the scratch space kept between builds,
# and /out where "Pocket Native Android" is put together: the game's app,
# for scripts/android/install.sh to put on the handheld with adb, and the
# program and libraries scripts/android/probe.sh runs from adb shell.
set -euo pipefail

FONT_URL=https://raw.githubusercontent.com/azahar-emu/azahar/c9e1e946a0b5d30a58ae14dee83c1f203a49622d/externals/open_source_archives/include/shared_font.app.romfs.h

rom="$1"
keys="${2:-}"
cd /src

case "${rom,,}" in
    *.cia)
        [ -n "$keys" ] || { echo "a .cia needs your console's aes_keys.txt, give it with --keys"; exit 1; }
        echo "== decrypting the .cia"
        python3 tools/cia2cci.py "$rom" /work/game.3ds "$keys"
        game=/work/game.3ds
        ;;
    *) game="$rom" ;;
esac
id=$(python3 tools/title_id.py "$game")
name=$(sed -n '/^name = "/{s/^name = "\(.*\)"$/\1/p;q}' "titles/$id/profile.toml" 2>/dev/null || true)
echo "== $id ${name:-(no profile yet)}"

echo "== building the recompiler, and Pocket Native for Android"
cargo build --release --locked -p recomp3ds --bin 3dsrecomp
cargo build --release --locked --target aarch64-linux-android -p pocket-native

echo "== recompiling the game for Android"
# the title's code is compiled with the NDK's clang against bionic, not with
# the ArkOS build's gcc against glibc, so the same title.so does not do for both
overrides=()
[ -d "titles/$id/overrides" ] && overrides=(--overrides "titles/$id/overrides")
RECOMP_CFLAGS=-DRECOMP_FASTMEM CC="$CC_aarch64_linux_android" \
    "$CARGO_TARGET_DIR/release/3dsrecomp" build "$game" "/work/recomp-android/$id" "${overrides[@]}"

if [ ! -f /work/shared_font.bin ]; then
    echo "== making the system font"
    curl -sSfL "$FONT_URL" -o /work/shared_font.app.romfs.h
    python3 tools/make_shared_font.py /work/shared_font.app.romfs.h /work/shared_font.bin
fi

echo "== putting Pocket Native Android together"
pn="/out/Pocket Native Android/pocket-native"
mkdir -p "$pn/games/$id/sysdata"
strip="$NDK_BIN/llvm-strip"
"$strip" -o "$pn/pocket-native" "$CARGO_TARGET_DIR/aarch64-linux-android/release/pocket-native"
rm -rf "$pn/titles" && cp -r titles "$pn/titles"
cp scripts/android/device/probe.sh "$pn/"
cp LICENSE CREDITS.md assets/fonts/OFL.txt "$pn/"
python3 tools/third_party.py "$pn/THIRD_PARTY_LICENSES.txt" >/dev/null
"$strip" -o "$pn/games/$id/title.so" "/work/recomp-android/$id/$id.so"
cp /work/shared_font.bin "$pn/games/$id/sysdata/shared_font.bin"
cp "$game" "$pn/games/$id/game.3ds"
chmod +x "$pn/pocket-native" "$pn/probe.sh"

echo "== packaging the app"
"$strip" -o /work/libpocket_native.so "$CARGO_TARGET_DIR/aarch64-linux-android/release/libpocket_native.so"
# the same key for every build, kept with the work, so that a new build
# installs over the last one
package=$(bash scripts/android/make_apk.sh "$id" "${name:-$id}" /work/libpocket_native.so "$pn/games/$id/title.so" \
    "$pn/games/$id/pocket-native.apk" /work/pocket-native.keystore | tail -1)
echo "$package" > "$pn/games/$id/package.txt"
echo "== done: scripts/android/install.sh $id puts ${name:-$id} on the handheld over adb"
