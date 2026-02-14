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
- Crash Team Racing: Locks up after displaying (broken) title screen
- Diablo: Mostly OK
- Dragon Warrior 7: Mostly ok, some flickering and audio popping
- Final Fantasy 7: Startup ok, gameplay looks quite broken.
- Final Fantasy 8: Intro is OK, some sound issues (vibrato?), opening cutscene is stuttery and has glitchy sound
- Final Fantasy 9: Shows a title with some glitchy graphics then tries to audio seek
- Final Fantasy Tactics: Mostly OK but menu seems to lock up after a short while.
- Front Mission 3: Mostly OK, some issues with an early cutscene, crashes trying to div 0.
- Grand Theft Auto 2: Loads up OK, a few framebuffer issues
- Gran Turismo: Mostly OK
- Gran Turismo 2: Tries to load from 1F801130 (just beyond the timers)
- Metal Gear Solid: Mostly OK, some blending issues and flickering
- PaRappa the Rapper: Loads up, sound OK, videos don't play
- Resident Evil: Mostly OK, backgrounds wrap weirdly on-screen.
- Resident Evil 2: Mostly OK, intro cutscene is a bit broken if not skipped. Intro video is OK. gameplay seems OK
- Silent Hill: Uses multitap memcard then locks up
- Spyro the Dragon: Lots of completely broken textures, sound in intro cutscene doesn't play
- Suikoden: Mostly ok, flickering at top of screen.
- Suikoden 2: Issues with CD changing mode while reading
- Tekken 3: Mostly ok!
- Tony Hawk's Pro Skater 2: Shows some video then locks up (copy protection?) (got a bit further than before?)
- Twisted Metal: Tries to play CD (lots of audio tracks)
- Vagrant Story: Mostly ok before crash due to VRAM copy issue
- Wild ARMS: Mostly ok!
- WipEout: Opening titles OK (offset a bit wrong), tries to access serial port

## TODO list

### Core

- Analog controllers
- Sound:
  - Missing CD Audio
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