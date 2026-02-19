# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Ape Escape: Requires dualshock
- Boku no Natsuyasumi: Doesn't boot
- Castlevania: Symphony of the Night: Titles ok, in-game UI is black, characters appear incorrectly lit
- Chrono Cross: Mostly OK, some flickering, title screen has some issues
- Chrono Trigger: Mostly OK
- Crash Bandicoot: Mostly ok
- Crash Bandicoot 2: Mostly ok
- Crash Bandicoot 3: Mostly ok
- Crash Team Racing: Mostly OK, some seams and occasional flickering (depth issue?) also image is not centred, large black bar at bottom.
- Diablo: Mostly OK
- Dino Crisis: Shows first screen then locks up
- Dragon Warrior 7: Mostly ok, some flickering and audio popping
- Final Fantasy 7: Mostly ok, some flickering
- Final Fantasy 8: Intro is OK, some sound issues (vibrato?), opening cutscene is stuttery and has glitchy sound
- Final Fantasy 9: Broken cutscenes
- Final Fantasy Tactics: Mostly OK
- Front Mission 3: Mostly OK
- Grand Theft Auto 2: Mostly OK
- Gran Turismo: Mostly OK
- Gran Turismo 2: Tries to load from 1F801130 (just beyond the timers)
- Metal Gear Solid: Mostly OK, some flickering
- PaRappa the Rapper: Mostly OK, some audio is a bit quiet
- Resident Evil: Mostly OK, blending issues
- Resident Evil 2: Mostly OK
- Silent Hill: Uses multitap memcard then locks up
- Spiderman: Gameplay looks weird when polys get close to camera
- Spyro the Dragon: Mostly OK
- Suikoden: Mostly ok, flickering at top of screen.
- Suikoden 2: Mostly OK
- Tekken 3: Mostly ok!
- Tomb Raider: Mostly ok
- Tony Hawk's Pro Skater: Gameplay is quite broken, level geometry is quite broken when close to camera.
- Tony Hawk's Pro Skater 2: Very similar to THPS1
- Twisted Metal: Mostly OK: gameplay looks a little weird maybe, eventually uses GP0 command 0x08BD0040 (???)
- Vagrant Story: Mostly ok - looks like outline of map in bottom right of gameplay blends with itself
- Wild ARMS: Mostly ok!
- WipEout: Opening titles OK, tries to access serial port

## TODO list

### Core

- Analog controllers
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