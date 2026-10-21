# Hardware licence options

Written for: the owner, who must choose a licence for the hardware design files.

> **Decided 2026-10-22:** CERN-OHL-P-2.0, following the recommendation below. The full licence text is not yet in the repository. See `hardware/LICENSE.md`.

## What needs a licence

The hardware work is in the repository as design files: schematics, PCB layouts, bills of materials, enclosure drawings and 3D models. None of these exist yet, except the bench BOM and the wiring notes in `hardware/`. The licence chosen will apply to these files.

Software is licensed under MIT OR Apache-2.0, as in `LICENSE-MIT` and `LICENSE-APACHE`. The question is whether the hardware should use the same permissive model, a reciprocal one, or a content licence.

## Options

### CERN-OHL-P-2.0 (Permissive)

A hardware licence written by CERN. Its "P" version is permissive.

- Anyone may use, modify and sell hardware made from the design.
- Modified designs do not have to be released.
- Attribution and the licence notice must be kept.
- Fits the project's permissive software licence.

### CERN-OHL-W-2.0 (Weakly reciprocal)

- Changes to the licensed files must be released under the same licence.
- Larger systems that include the licensed files can be proprietary.
- A middle ground between the permissive and strongly reciprocal versions.

### CERN-OHL-S-2.0 (Strongly reciprocal)

- Any derivative design must be released under the same licence, including the whole system.
- The strictest of the three, and the least like the project's software licence.

### CC-BY-4.0 (Creative Commons Attribution)

- A licence written for creative works, such as text, images and music.
- Used by some projects for documentation, but it has no terms written for design files. For example, it does not say how source files must be provided.
- Creative Commons advises against its use for software. Check whether that advice also covers hardware before using it for design files.

### MIT or Apache-2.0 for the hardware files

- The software licences the project already uses.
- Simple and widely understood.
- Neither licence was written for hardware, so questions about source files and manufacturing are not addressed.

### TAPR Open Hardware License

- A hardware licence from the TAPR amateur radio group.
- Permissive in intent, but its terms were not reviewed for this memo.

## Comparison

| Licence | Modifications must be shared | Hardware-specific terms | Fit with project stance |
|---|---|---|---|
| CERN-OHL-P-2.0 | No | Yes | Closest |
| CERN-OHL-W-2.0 | Changes to licensed files | Yes | Middle |
| CERN-OHL-S-2.0 | Whole derivative design | Yes | Least close |
| CC-BY-4.0 | Not required | No | Poor for design files |
| MIT or Apache-2.0 | Not required | No | Matches software, but not written for hardware |
| TAPR OHL | Not reviewed | Yes | Not reviewed |

## Recommendation (for the owner to decide)

Use CERN-OHL-P-2.0 for the hardware design files. It is written for hardware, it is permissive like the software licence, and it keeps the same attribution expectations.

Keep documentation under the project's existing licence, as the rest of `docs/` already is.

Before adopting this, check the official CERN-OHL-P-2.0 text, and confirm that the owner accepts its permissive terms.

## Decision record

| Date | Decision | Decided by |
|---|---|---|
| 2026-10-22 | CERN-OHL-P-2.0 for the hardware design files. Documentation stays under the project's licence. | Owner, by instruction to follow the recommendations |
