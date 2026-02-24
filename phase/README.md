# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Ape Escape: Mostly OK, some missing polys, sound is a bit off
- Boku no Natsuyasumi: Mostly OK
- Castlevania: Symphony of the Night: Titles ok, in-game UI is black, characters appear incorrectly lit
- Chrono Cross: Mostly OK, some flickering, title screen has some issues
- Chrono Trigger: Mostly OK
- Crash Bandicoot: Mostly ok
- Crash Bandicoot 2: Mostly ok
- Crash Bandicoot 3: Mostly ok
- Crash Team Racing: Mostly OK, some seams and occasional flickering (depth issue?) also image is not centred, large black bar at bottom.
- Diablo: Mostly OK
- Dino Crisis: Shows first screen then locks up
- Dino Crisis 2: Shows initial titles, then plays some audio without video and then locks up
- Dragon Warrior 7: Mostly ok, some flickering and audio popping
- Final Fantasy 7: Mostly ok, some flickering, intro titles have slightly off sound (high-freq issue?)
- Final Fantasy 8: Intro is OK, some sound issues (vibrato?), opening cutscene is stuttery and has glitchy sound, gameplay is OK but sound issues
- Final Fantasy 9: Broken cutscenes
- Final Fantasy Tactics: Mostly OK
- Front Mission 3: Mostly OK
- Grand Theft Auto 2: Mostly OK
- Gran Turismo: Mostly OK
- Gran Turismo 2: Tries to load from 1F801130 (just beyond the timers)
- Legend of Dragoon: Mostly OK (?)
- Medal of Honor: Mostly OK
- Metal Gear Solid: Mostly OK, some flickering
- Metal Slug X: Shows some opening screens then locks up.
- PaRappa the Rapper: Mostly OK, some audio is a bit quiet
- Parasite Eve: Mostly OK (?)
- Resident Evil: Mostly OK
- Resident Evil 2: Mostly OK
- Resident Evil 3: Intro cutscene loses sound, VRAM write crash when gameplay begins
- Silent Hill: Uses multitap memcard then locks up
- Sim City 2000: Does strange stuff with MDEC, does not boot.
- Spiderman: Mostly OK
- Spyro the Dragon: Mostly OK
- Suikoden: Mostly ok, flickering at top of screen.
- Suikoden 2: Mostly OK
- Tekken 3: Mostly ok!
- Tomb Raider: Mostly ok, music in game selection screen does not play
- Tony Hawk's Pro Skater: Mostly OK, sometimes locks up when loading level
- Tony Hawk's Pro Skater 2: Very similar to THPS1
- Twisted Metal: Mostly OK: gameplay looks a little weird maybe, eventually uses GP0 command 0x08BD0040 (???)
- Vagrant Story: Mostly ok - looks like outline of map in bottom right of gameplay blends with itself
- Wild ARMS: Mostly ok!
- WipEout: Mostly OK!

## TODO list

### Core

- Graphics fixes:
  - Texture mapping
  - Interpolation
  - And more...

### Other

- Hardware rendering
- Performance improvements
- Save states
- Cache emulation
- PAL support
- External controller support