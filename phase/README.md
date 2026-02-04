# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: If left for a long time, plays Konami title then locks up.
- Crash Bandicoot: Boots, everything with negative X coords looks broken.
- Crash Bandicoot 2: Crashes trying to draw line
- Crash Bandicoot 3: Peripheral error (multitap?)
- Crash Team Racing: Locks up after displaying (broken) title screen
- Final Fantasy 7: Slightly broken titles, intro video plays, gameplay looks quite broken.
- Final Fantasy Tactics: Flashes a bit (titles), video. It did segfault once here (???). Actually gets into gameplay OK, with some jank.
- Gran Turismo: Crashes trying to use nonsense GPU command. If this is ignored, crashes copying frame buffer
- Metal Gear Solid: Waiting for SPU interrupt.
- Resident Evil: Write VRAM block out of bounds (might require wrapping)
- Resident Evil 2: Displays titles then video, then black screen and lockup
- Suikoden: Displays video then crashes due to draw line.
- Tekken 3: Displays a title then crashes due to VRAM overflow.
- Tony Hawk's Pro Skater 2: Shows some video then locks up (copy protection?)
- Wild ARMS: Titles, displays video, menu looks OK, then line drawing crash.

## TODO list

### Core

- SPU
    - CD streaming audio
- MDEC fixes
- Memory cards
- Analog controllers

### Other

- Hardware rendering
- Performance improvements
- Save states
- Cache emulation
- PAL support