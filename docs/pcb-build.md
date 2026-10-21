# Custom PCB: assembly and build instructions

Status: **BLOCKED.** These instructions cannot be written until the schematic exists.

The custom board is planned in `todo.md`, Phase 5. It needs a schematic, a layout, and a bill of materials. None of these exist yet. The build environment has no PCB or CAD tool, so no design files can be created or checked here.

## What must exist first

1. A KiCad schematic, covering the power input, protection, the ESP32-S3 module, the GNSS receiver and the antenna connector.
2. A PCB layout that passes design-rule and electrical checks.
3. Gerber, drill, bill-of-materials and pick-and-place output.
4. A prototype order and bring-up of the first board.

Once those exist, these instructions will cover ordering, assembly, first power-up, and the checks to run before the board touches a vehicle.

## What is not decided

- The power design for 12 V and 24 V input, including transient and reverse-polarity protection.
- The battery backup and the shutdown behaviour.
- The antenna placement.

See the hardware licence options in [`hardware-licence-options.md`](hardware-licence-options.md) for how the design files will be licensed.
