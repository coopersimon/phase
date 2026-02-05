# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: If left for a long time, plays Konami title then locks up.
- Crash Bandicoot: Boots, everything with negative X coords looks broken.
- Crash Bandicoot 2: Crashes trying to draw line
- Crash Bandicoot 3: Peripheral error (multitap?)
- Crash Team Racing: Locks up after displaying (broken) title screen
- Dragon Warrior 7: Shows intro title video then locks up
- Final Fantasy 7: Slightly broken titles, intro video plays, gameplay looks quite broken.
- Final Fantasy 8: Shows titles, doesn't allow skip.
- Final Fantasy 9: Shows a title with some glitchy graphics then tries to audio seek
- Final Fantasy Tactics: Titles + video. Actually gets into gameplay OK, with some jank.
- Grand Theft Auto 2: Starts ok
- Gran Turismo: Crashes trying to use nonsense GPU command. If this is ignored, plays intro video, unskippable(?), then title looks broken.
- Metal Gear Solid: Waiting for SPU interrupt? Shows a few titles then locks up.
- Resident Evil: Write VRAM block out of bounds (might require wrapping)
- Resident Evil 2: Displays titles then video, then black screen and lockup
- Suikoden: Displays video then crashes due to draw line.
- Tekken 3: Displays a title then crashes due to VRAM overflow.
- Tony Hawk's Pro Skater 2: Shows some video then locks up (copy protection?)
- Vagrant Story: Shows intro video with some glitches, then freezes on broken title screen
- Wild ARMS: Titles, displays video, menu looks OK, then line drawing crash.

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