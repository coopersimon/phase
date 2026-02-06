# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: If left for a long time, plays Konami title then locks up.
- Chrono Trigger: Intro video plays, then crashes writing to VRAM.
- Crash Bandicoot: Boots, everything with negative X coords looks broken.
- Crash Bandicoot 2: Boots, startup looks kinda ok, a bit choppy FPS, some lighting issues
- Crash Bandicoot 3: Boots, generally OK
- Crash Team Racing: Locks up after displaying (broken) title screen
- Dragon Warrior 7: Shows intro title video then locks up
- Final Fantasy 7: Slightly broken titles, intro video plays, gameplay looks quite broken.
- Final Fantasy 8: Loads up mostly OK, graphics glitches in gameplay (clipping with bottom of frame.)
- Final Fantasy 9: Shows a title with some glitchy graphics then tries to audio seek
- Final Fantasy Tactics: Titles + video. Actually gets into gameplay OK, with some jank.
- Grand Theft Auto 2: Loads up OK, a few framebuffer issues
- Gran Turismo: Generally OK, but a lot of nonsense GPU commands that should do stuff. Locks up when trying to enter a race
- Metal Gear Solid: Waiting for SPU interrupt? Shows a few titles then locks up.
- Resident Evil: Write VRAM block out of bounds (might require wrapping)
- Resident Evil 2: Starts OK, lighting issues
- Suikoden: Displays video then doesn't want to continue due to memcard issues.
- Tekken 3: Issue with peripheral interface
- Tony Hawk's Pro Skater 2: Shows some video then locks up (copy protection?) (got a bit further than before?)
- Vagrant Story: Titles and pre-video OK, title is broken, intro video issues, gameplay has glitches at left+bottom of screen
- Wild ARMS: Mostly ok!

## TODO list

### Core

- SPU
    - CD streaming audio
- Memory cards
- Analog controllers

### Other

- Hardware rendering
- Performance improvements
- Save states
- Cache emulation
- PAL support