# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: Is calling other CD commands while reading from CD.
- Crash Bandicoot: Boots, everything with negative X coords looks broken.
- Crash Bandicoot 2: Crashes trying to draw line
- Crash Bandicoot 3: Peripheral error (multitap?)
- Crash Team Racing: Locks up after displaying (broken) title screen
- Final Fantasy 7: Gets to main menu, displays broken video (24-bit color).
- Final Fantasy Tactics: Flashes a bit (titles), then displays broken video. Then crashes at menu (get loc L on CD)
- Gran Turismo: Crashes trying to use nonsense GPU command. If this is ignored, CD streaming issue (similar to castlevania, but potentially more complex)
- Metal Gear Solid: Waiting for SPU interrupt.
- Resident Evil: Write VRAM block out of bounds (might require wrapping)
- Resident Evil 2: After adjusting CD timings, no longer displays still image. So this is timing related.
- Suikoden: Displays partially broken video then locks up (timing?)
- Tekken 3: Displays a title then locks up (similar to castlevania)
- Tony Hawk's Pro Skater 2: Issue streaming from CD.
- Wild ARMS: Titles, displays broken 24-bit video, menu looks OK, then line drawing crash.

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