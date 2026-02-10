# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: If left for a long time, plays Konami title then locks up.
- Chrono Trigger: Intro video plays, then crashes writing to VRAM.
- Crash Bandicoot: Mostly ok
- Crash Bandicoot 2: Mostly ok, frame rate issues
- Crash Bandicoot 3: Now breaks due to peripheral issue.
- Crash Team Racing: Locks up after displaying (broken) title screen
- Dragon Warrior 7: Mostly ok, some perf issues
- Final Fantasy 7: Startup ok, gameplay looks quite broken.
- Final Fantasy 8: Loads up mostly OK, graphics glitches in gameplay (clipping with bottom of frame.)
- Final Fantasy 9: Shows a title with some glitchy graphics then tries to audio seek
- Final Fantasy Tactics: Titles + video: intro sound seems to lock things up. Actually gets into gameplay OK, with some jank.
- Grand Theft Auto 2: Loads up OK, a few framebuffer issues
- Gran Turismo: Generally OK, but a lot of nonsense GPU commands that should do stuff. Locks up when trying to enter a race
- Metal Gear Solid: Waiting for SPU interrupt? Shows a few titles then locks up.
- Resident Evil: Write VRAM block out of bounds (might require wrapping)
- Resident Evil 2: Mostly OK, intro cutscene is a bit broken if not skipped. Intro video is OK. gameplay seems OK
- Suikoden: Pretty good, issues with some clipped backgrounds/sprites behaving oddly at the top and left (they are getting shifted, not clipped)
- Tekken 3: Mostly ok!
- Tony Hawk's Pro Skater 2: Shows some video then locks up (copy protection?) (got a bit further than before?)
- Vagrant Story: Mostly ok before crash due to VRAM copy issue
- Wild ARMS: Mostly ok!

## TODO list

### Core

- Memory cards
- Analog controllers

### Other

- Hardware rendering
- Performance improvements
- Save states
- Cache emulation
- PAL support