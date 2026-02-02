# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: Appears to try to load ADPCM from CD, gets stuck waiting for a CD read 
- Crash Bandicoot: Boots, everything with negative X coords looks broken.
- Crash Bandicoot 2: Crashes trying to draw line
- Crash Bandicoot 3: Peripheral error (multitap?)
- Crash Team Racing: Tries to stream ADPCM from CD and locks up.
- Final Fantasy 7: Gets to main menu, then CD streaming issue.
- Final Fantasy Tactics: Flashes a bit (titles) then CD streaming issue.
- Gran Turismo: Crashes trying to use nonsense GPU command. If this is ignored, CD streaming issue.
- Metal Gear Solid: Waiting for SPU interrupt. Also segfaults (???)
- Resident Evil: Write VRAM block out of bounds (might require wrapping)
- Resident Evil 2: Displays a still image, then loops reading the CD. Seems to get stuck spamming cd interrupt?
- Suikoden: Crashes reading a nonsense address.
- Tekken 3: Some titles then MDEC issues. Seems able to stream from CD.
- Tony Hawk's Pro Skater 2: Issue streaming from CD.
- Wild ARMS: Some titles then streaming from CD. If skipped line drawing crash.

## TODO list

### Core

- SPU
- MDEC
- Memory cards
- Analog controllers

### Other

- Hardware rendering
- Performance improvements
- Save states
- Cache emulation
- PAL support