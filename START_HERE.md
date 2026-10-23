# Start here

Pocket Native does not work like an emulator. There is no app to install
on the handheld and no game included. A PC rebuilds your own game dumps
into a folder for the SD card, and that is the part most people miss.

The whole thing in one line: download the kit, drop your game on
`Build a game`, and copy the folder it makes to the card.

Tested and tuned: Pokémon Omega Ruby and Alpha Sapphire, Animal
Crossing: New Leaf and Mario Kart 7 (the US versions of those two).
Other games may or may not boot.

## What you need

- An R36S (or a close clone) with ArkOS. dArkOSRE works too. Other
  systems, dArkOS included, are untested.
- A PC for the build, with Windows or Linux:
  - Windows: Docker Desktop or Podman Desktop, running before you
    start. If neither is installed, the build offers to install Docker
    Desktop for you.
  - Linux: podman or Docker.
- Your own dump of the game: a decrypted `.3ds`/`.cci`, or a `.cia`
  along with your console's `aes_keys.txt`.

## Getting the kit

Open the [latest release](https://github.com/whovian64/pocket-native/releases/latest),
download **Source code (zip)** under Assets, and unzip it somewhere with
a few GB to spare. The folder you get is the kit, and everything below
happens in it. You only do this once: later versions come in through
the updater.

## Building

On Windows, drag your game onto `Build a game.bat` in the kit's folder,
or double-click it and pick the game. On Linux, do the same with
`Build a game`. Several games can go at once. The first time, Windows or
the file manager may ask whether to run it, see below if it does.

A `.cia` needs your console's `aes_keys.txt`. Put it next to the game
or in the kit's folder and the build finds it, otherwise it asks for it.

A window shows the build as it goes and stays open at the end, so you
can read how it went. The first build sets up the build container,
which downloads a few hundred MB and takes a while. Each game's code is
recompiled, which takes several minutes for a big game. Later builds
reuse what they can.

The result is a `Pocket Native` folder in the kit's `build` folder.
Building more games adds them to it, one launcher each. A game with no
profile yet builds with `(no profile yet)` after its name and is named
after its title ID.

From a terminal, in the kit's folder, the same build is:

```bash
scripts/build.sh path/to/game.3ds
```

and `scripts\build.ps1` in PowerShell on Windows. Both take several
games at once, and `--keys path/to/aes_keys.txt` (`-Keys` on Windows)
for a `.cia` whose keys are elsewhere.

## Installing

The build leaves a folder called `Pocket Native` in `build`. Inside it
are two parts, and they have to stay together:

- one `.sh` file per game, named after the game. That file is the
  launcher, and it is what EmulationStation lists.
- a folder called `pocket-native`, with the program and the games in
  it. The launcher starts the copy of `pocket-native` beside it, so
  neither part works on its own.

1. Copy the whole `Pocket Native` folder, as it is, into `ports` on
   the card's game partition: `roms/ports`, or `roms2/ports` with two
   cards. The result should look like this:

   ```
   roms/ports/Pocket Native/Pokémon Omega Ruby.sh
   roms/ports/Pocket Native/pocket-native/
   ```

   Do not copy the `build` folder itself, do not copy just one part,
   and do not rename anything or move files between the two parts.
2. Put the card back in the handheld and restart EmulationStation.
3. Each game shows up under Ports, named after the game, one entry
   per `.sh` file.

Building another game later adds its `.sh` file and its game files to
the same output folder. Copy the folder to the card again and let it
overwrite and merge, so the new game arrives whole.

If a game is missing from Ports, count the `.sh` files where you
copied the folder: there should be one per game, sitting directly in
`Pocket Native`, not inside `pocket-native`. If a game is listed but
does not start, the folder arrived in pieces: delete it from the card
and copy the whole folder again.

A tap of FN opens the menu in game and pauses it, FN + Start quits. On a handheld with no FN, press L3 and R2 together for the menu, and use L3 in place of FN for the rest.
Saves and the game's log go in
`Pocket Native/pocket-native/games/<title id>/data`.

## Updating

When a new version is out, the build says so when it finishes. Open
`Update Pocket Native` in the kit's folder (`Update Pocket Native.bat`
on Windows). It downloads the new version into the kit, keeps the old
files in `.old`, and builds every game in `build` again from the copies
the last build kept there, so you don't need your game files for it.
Then copy the `Pocket Native` folder onto the card again, over the old
one. Your saves stay where they are.

The menu shows which version you are running, next to "Pocket Native"
at the top.

Coming from 0.1.0 or 0.1.1, which have no updater yet: get the new kit
as above, build your games with it, and copy the folder over the old
one on the card. From then on the updater does it.

## If it goes wrong

**Windows says it protected your PC, or asks whether to run the file.**
It does that for files that came from the internet. Click **More info**
and then **Run anyway**, or **Run**.

**Nothing happens when you open `Build a game` on Linux.** The file
manager does not run launchers it does not trust. Allow it when it
asks, or in the file's properties (allow executing, or allow
launching), or run `scripts/build.sh` from a terminal instead.

**The build says the image is encrypted.** The dump has to be decrypted
first, or built from the `.cia` with its `aes_keys.txt` instead.

**The Windows build cannot reach Docker.** Docker Desktop is not
running. Start it, let it finish starting, and run the build again.

**The update says it could not reach GitHub.** Check the PC's internet
connection and try again, or get the new kit by hand as above.

**The game is not under Ports.** The folder is in the wrong place, or
EmulationStation has not been restarted. The whole `Pocket Native`
folder goes inside `ports`, not just the files in it. If it still does
not show up, your system may only list launchers sitting directly in
`ports`: move the `.sh` files and the `pocket-native` folder out of
`Pocket Native` into `ports` itself, side by side.

**A black screen with choppy sound, and FN pauses but shows no menu.**
The handheld's GPU did not start, so the game is drawing in software,
far too slowly to show anything. No setting fixes that. Builds before
0.1.1 did this on dArkOS and dArkOSRE, so build the game again with the
current version. If it still happens, the game's `log.txt` has a line
with `could not initialize EGL`, please post it in an issue.

**A game that is not one of the tested ones does not start, or runs
badly.** That is expected for now. Its `log.txt`, in the game's
`data` folder above, says what the emulator ran into. Most games have
something worth trying first: open the menu, go to **Advanced
settings**, pick **Most compatible**, then **Restart game**.
