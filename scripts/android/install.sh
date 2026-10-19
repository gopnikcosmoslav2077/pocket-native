#!/bin/bash
# install.sh <title id> [--out dir] [--serial id] [--no-start] [--internal | --sd <card>]
#
# Puts a game scripts/android/build.sh made on the handheld with adb: installs
# its app, pushes the system font and the profiles to the app's files folder
# and the game to the app's own folder on the SD card, or with no card (or
# --internal) beside the rest (later pushes skip what has not changed), and
# starts it. --sd names the card, as /storage lists it, where there are two.
# USB debugging has to be on. The app's log is then in
# /sdcard/Android/data/<package>/files/data/log.txt, which --log pulls.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
out="$repo/build" id="" adb=(adb) start=1 log_only=0 card="" internal=0
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="$(realpath -m "$2")"; shift 2 ;;
        --serial) adb=(adb -s "$2"); shift 2 ;;
        --no-start) start=0; shift ;;
        --log) log_only=1; shift ;;
        --internal) internal=1; shift ;;
        --sd) card="$2"; shift 2 ;;
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) id="$1"; shift ;;
    esac
done
[ -n "$id" ] || { echo "usage: scripts/android/install.sh <title id>"; exit 2; }
game="$out/Pocket Native Android/pocket-native/games/$id"
pn="$out/Pocket Native Android/pocket-native"
[ -f "$game/pocket-native.apk" ] || { echo "no app for $id in $game, build it with scripts/android/build.sh first"; exit 2; }
command -v adb >/dev/null || { echo "adb is not installed (Android platform-tools)"; exit 1; }
package=$(cat "$game/package.txt")
files="/sdcard/Android/data/$package/files"
"${adb[@]}" wait-for-device

if [ $log_only = 1 ]; then
    "${adb[@]}" pull "$files/data/log.txt" "$out/log-$id.txt" >/dev/null && echo "== log in $out/log-$id.txt"
    "${adb[@]}" pull "$files/data/panic.txt" "$out/panic-$id.txt" >/dev/null 2>&1 && echo "== and a panic, in $out/panic-$id.txt"
    exit 0
fi

echo "== installing $package"
# copied whole before installing, the streamed install can stall
"${adb[@]}" install --no-streaming -r "$game/pocket-native.apk" >/dev/null
"${adb[@]}" shell mkdir -p "$files/sysdata" "$files/titles"
"${adb[@]}" push --sync "$game/sysdata/." "$files/sysdata/" >/dev/null
"${adb[@]}" push --sync "$pn/titles/." "$files/titles/" >/dev/null

# the game, the big one, on the SD card when there is one: a card is what
# /storage lists besides the built-in storage, its name its volume id
if [ $internal = 0 ] && [ -z "$card" ]; then
    card=$("${adb[@]}" shell ls /storage | tr -d '\r' | grep -v -x -e emulated -e self | head -1 || true)
fi
target="$files"
if [ $internal = 0 ] && [ -n "$card" ]; then
    target="/storage/$card/Android/data/$package/files"
    if ! "${adb[@]}" shell ls -d "$target" >/dev/null 2>&1; then
        # the app makes its own folder on the card the first time it starts
        "${adb[@]}" shell am start -W -n "$package/android.app.NativeActivity" >/dev/null 2>&1 || true
        sleep 2
        "${adb[@]}" shell am force-stop "$package" >/dev/null 2>&1 || true
        "${adb[@]}" shell mkdir -p "$target"
    fi
    echo "== pushing the game to the SD card ($card), unchanged files are skipped"
else
    echo "== pushing the game to the built-in storage, unchanged files are skipped"
fi
"${adb[@]}" push --sync "$game/game.3ds" "$target/game.3ds" >/dev/null
if [ "$target" != "$files" ]; then
    # a copy from before there was a card, which the card's now stands for
    "${adb[@]}" shell rm -f "$files/game.3ds"
fi
if [ $start = 1 ]; then
    echo "== starting it"
    "${adb[@]}" shell am start -n "$package/android.app.NativeActivity" >/dev/null
fi
echo "== done; scripts/android/install.sh $id --log pulls its log"
