#!/bin/bash
# install.sh <title id> [--out dir] [--serial id] [--no-start]
#
# Puts a game scripts/android/build.sh made on the handheld with adb: installs
# its app, pushes the game, the system font and the profiles to the app's
# files folder (later pushes skip what has not changed), and starts it.
# USB debugging has to be on. The app's log is then in
# /sdcard/Android/data/<package>/files/data/log.txt, which --log pulls.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
out="$repo/build" id="" adb=(adb) start=1 log_only=0
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="$(realpath -m "$2")"; shift 2 ;;
        --serial) adb=(adb -s "$2"); shift 2 ;;
        --no-start) start=0; shift ;;
        --log) log_only=1; shift ;;
        -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
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
"${adb[@]}" install -r "$game/pocket-native.apk" >/dev/null
echo "== pushing the game, unchanged files are skipped"
"${adb[@]}" shell mkdir -p "$files/sysdata" "$files/titles"
"${adb[@]}" push --sync "$game/game.3ds" "$files/game.3ds" >/dev/null
"${adb[@]}" push --sync "$game/sysdata/." "$files/sysdata/" >/dev/null
"${adb[@]}" push --sync "$pn/titles/." "$files/titles/" >/dev/null
if [ $start = 1 ]; then
    echo "== starting it"
    "${adb[@]}" shell am start -n "$package/android.app.NativeActivity" >/dev/null
fi
echo "== done; scripts/android/install.sh $id --log pulls its log"
