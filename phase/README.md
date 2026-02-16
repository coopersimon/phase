# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Ape Escape: Requires dualshock
- Boku no Natsuyasumi: Doesn't boot
- Castlevania: Symphony of the Night: If left for a long time, plays Konami title then locks up.
- Chrono Cross: Mostly OK, some blending issues.
- Chrono Trigger: Starts OK, some 2d graphics glitches, then locks up before gameplay begins
- Crash Bandicoot: Mostly ok
- Crash Bandicoot 2: Mostly ok
- Crash Bandicoot 3: Mostly ok
- Crash Team Racing: Locks up after displaying title screen
- Diablo: Mostly OK
- Dino Crisis: Shows first screen then locks up
- Dragon Warrior 7: Mostly ok, some flickering and audio popping
- Final Fantasy 7: Mostly ok, some flickering, characters appear with a bluish tint
- Final Fantasy 8: Intro is OK, some sound issues (vibrato?), opening cutscene is stuttery and has glitchy sound
- Final Fantasy 9: Shows a title with some glitchy graphics then tries to audio seek
- Final Fantasy Tactics: Mostly OK but menu seems to lock up after a short while.
- Front Mission 3: Mostly OK
- Grand Theft Auto 2: Loads up OK, a few framebuffer issues
- Gran Turismo: Mostly OK
- Gran Turismo 2: Tries to load from 1F801130 (just beyond the timers)
- Metal Gear Solid: Mostly OK, some blending issues and flickering
- PaRappa the Rapper: Loads up, sound OK, videos don't play
- Resident Evil: Mostly OK, blending issues
- Resident Evil 2: Mostly OK, intro cutscene is a bit broken if not skipped. Intro video is OK. gameplay seems OK
- Silent Hill: Uses multitap memcard then locks up
- Spiderman: Similar to THPS 1+2, audio sounds particularly bad.
- Spyro the Dragon: Mostly OK
- Suikoden: Mostly ok, flickering at top of screen.
- Suikoden 2: Shows a title then plays audio without a video, and locks up
- Tekken 3: Mostly ok!
- Tomb Raider: Mostly ok
- Tony Hawk's Pro Skater: Shows some video then locks up. If skipped quickly, menus are ok, but gameplay is totally broken.
- Tony Hawk's Pro Skater 2: Very similar to THPS1, except character select menu looks off.
- Twisted Metal: Mostly OK: gameplay looks a little weird maybe, eventually uses GP0 command 0x08BD0040 (???)
- Vagrant Story: Mostly ok before crash due to VRAM copy issue
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