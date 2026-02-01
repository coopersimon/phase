# phase

This is a PlayStation emulator. It is in development. It does software rendering.

## Game list

Below is a list of selected games and their status.

- Castlevania: Symphony of the Night: Appears to try to load ADPCM from CD, gets stuck waiting for a CD read 
- Crash Bandicoot: Boots, everything with negative X coords looks broken.
- Crash Bandicoot 2: Crashes trying to draw line
- Crash Bandicoot 3: Peripheral error (multitap?)
- Crash Team Racing: Tries to stream ADPCM from CD and locks up.
- Final Fantasy 7: MDEC crash
- Final Fantasy Tactics: Flashes a bit then MDEC crash.
- Gran Turismo: Crashes trying to use nonsense GPU command. If this is ignored, MDEC crash.
- Metal Gear Solid: Waiting for SPU interrupt. Also segfaults (???)
- Resident Evil: MDEC crash
- Resident Evil 2: MDEC crash
- Suikoden: MDEC crash
- Tekken 3: Some titles then MDEC crash.
- Tony Hawk's Pro Skater 2: MDEC crash
- Wild ARMS: Some titles then MDEC crash. If skipped line drawing crash.

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