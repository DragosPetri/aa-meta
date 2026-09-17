---
name: configure-adc
description: Guide the user through configuring an ADC device in an attach-meta workfile — channel setup, reference voltage, sampling rate, gain, and interface wiring
---

# Configure ADC

You are helping the user configure an ADC (Analog-to-Digital Converter) device in an attach-meta workfile. This skill provides ADC domain knowledge — what properties mean, what valid values are, and the tradeoffs involved. The exact command invocations (how to add a node, how to set a property, what path segments to use) come from the tool's manifest, which you must always re-read rather than assume.

This skill assumes the workfile already exists (Phase 2 of `attach-setup` is underway). If it does not, return to `attach-setup` to create it first.

---

## Before touching anything — read the manifest and the current state

### Read the manifest

Locate the registered tool's manifest path in `.attach-meta.toml` and read it. For every command you are about to use (`add`, `read`, `update`, `validate`, …), check its `CommandMapping`:

- `description` — the tool author's explanation of what the command does in this tool's context; use this wording when explaining actions to the user
- `argv` — the actual binary invocation
- `args` — any extra flags the tool declares beyond the protocol base

Re-read the manifest each time you are unsure of a command's behavior or argument shape. Never rely on memory of a previous read.

### Read the current device state

```
attach-meta --json read <device-key>
```

Parse the `ReadResponse` (read `attach-meta schema responses` for the discriminator and rendering hints). The response reveals whether this is a **fresh** device or an **existing** one:

- **Fresh**: the node has no properties set. Proceed through all topics below in order.
- **Existing**: some properties are already set. Build a status table showing each topic, its current value, and whether it looks complete. Ask the user: "Everything looks good — what would you like to change?" or, if gaps are visible, "These fields are missing — want me to walk through them?"

In both cases use the same topic flow. For an existing device, skip topics where the current value is already present and the user confirms it is correct. Never overwrite an existing value without showing it to the user first.

---

## Topic 1 — Device identity

If not already visible in the read output (e.g. a `compatible` or `model` property), ask the user which ADC they are configuring. The type determines which topics are relevant and what values are valid:

- **Sigma-delta ADCs** (AD4130, AD7124, AD7173, AD7768, …): programmable gain amplifier, selectable digital filters (sinc3/sinc5/FIR), ODR derived from filter and decimation.
- **SAR ADCs** (AD7682, AD7689, AD4000-family, …): fixed throughput rate, typically no on-chip PGA, simpler reference configuration.
- **Simultaneous-sampling ADCs** (AD7768-4, AD4134, …): all channels sample at the same instant; channel enable/disable affects throughput.

---

## Topic 2 — Channels

Check the read output for existing channel nodes first. For each channel the user wants to configure:

1. **Create the channel node** using `add`. Re-read the manifest's `add` description to confirm the exact flags for naming and placement — the tool defines how channels are named and where they live in the tree.

2. Ask: single-ended or differential?
   - **Differential**: two analog inputs (positive and negative). Ask for both pin assignments.
   - **Single-ended**: one input referenced to GND or an internal node.
   - **Pseudo-differential**: one input referenced to a low-side input that is not GND.

3. Ask for the channel index (0-based) and, if the device uses an analog mux, the input pin numbers.

4. Ask whether the channel should be enabled.

5. **Set properties** using `update`. Re-read the manifest's `update` description for the exact path convention and `--with` syntax. The `update` command behaves as an upsert — it inserts the property if it does not exist.

After each write, run `read` on the channel node to confirm the value was stored correctly.

---

## Topic 3 — Reference voltage

Check the read output for an existing reference configuration first. Ask the user:

1. **Reference source**:
   - `internal` — on-chip reference (e.g. 2.5 V on many ADI parts); no external component needed
   - `external` — REFIN+/REFIN− pins; ask for the voltage in mV (e.g. 5000, 4096)
   - `avdd` — AVDD used as reference

2. **Voltage value** — required when source is `external`; used by drivers to compute LSB weight and physical units.

Some devices (AD4130, AD7124-style) set the reference per channel rather than globally. If the read output shows per-channel reference properties, set them there instead.

Use `update` to write. Re-read the manifest's `update` description for path conventions.

---

## Topic 4 — Gain / PGA

Skip for devices without a programmable gain amplifier (most SAR ADCs). Check the read output for existing gain settings first.

Ask the user:
- Expected peak input voltage
- Reference voltage

Suggest a gain: `gain = V_ref / (2 × V_in_peak)` for bipolar input, rounded down to the nearest available power-of-two. Common values: 1, 2, 4, 8, 16, 32, 64, 128. Warn that gains exceeding the device's noise-free resolution limit do not improve accuracy.

Use `update` to write. Re-read the manifest's `update` description for the exact path.

---

## Topic 5 — Output data rate / sampling rate

Check the read output for existing ODR settings first.

For **sigma-delta ADCs**:
- Ask the user what sample rate (in SPS) they need.
- Tradeoff: lower ODR = more averaging = lower noise (better ENOB); higher ODR = faster updates = more noise.
- Common filter types: `sinc3` (general purpose), `sinc5` (lower latency), `fir` (flat passband, more latency), `post_filter` (50/60 Hz notch for industrial).
- ODR and filter type may be set together or separately depending on the tool — re-read the manifest.

For **SAR ADCs**:
- Ask for the desired throughput in SPS.
- Warn if it exceeds the part's maximum specified throughput.

Use `update` to write. Re-read the manifest's `update` description for path conventions.

---

## Topic 6 — SPI interface

**Bus and chip-select are structural** — they are set when the device node is placed in the workfile and are already fixed if the device exists. Skip asking for them on an existing device; only raise them when adding a brand-new device node.

**`reg` and the node name are coupled**: the unit address in the node name (the part after `@`, e.g. `adc@0`) must equal the `reg` property value. Whenever `reg` is set or changed, the node must be renamed to match, and vice versa — mismatches are a DTS error. Verify the current node name against `reg` when reviewing an existing device, and keep them in sync on any write.

For both fresh and existing devices, the remaining SPI properties (max clock speed and mode) may still need to be set. Check the read output first:

- **Max SPI clock**: most ADI SAR ADCs support 50–80 MHz; sigma-delta parts are often 10–20 MHz — check the datasheet
- **SPI mode**: most ADI ADCs use mode 3 (CPOL=1, CPHA=1) or mode 0 — check the datasheet

Use `update` to write. Re-read the manifest's `update` description for path conventions.

---

## Topic 7 — Interrupts / DOUT/RDY

Check the read output for existing interrupt settings first. If the device has a data-ready pin (DOUT/RDY, /RDY, INT), ask the user which GPIO line is connected and the trigger type (typically falling edge, active-low). Check available intelligence for additional properties that indicate which interrupt controller owns the line and ask the user for those values as well.

Use `update` to write. Re-read the manifest's `update` description for path conventions.

---

## Topic 8 — Validate

After all properties are set — or after any individual change on an existing device — run:
```
attach-meta --json validate <device-key>
```

Parse `ValidationResponse` — it has no `ok` field; a non-empty `errors` array means invalid. Show all errors and warnings. For each error, suggest a fix using the ADC domain context above (e.g. missing reference source, out-of-range gain, unsupported ODR for the selected filter).

---

## Using intelligence

Run `attach-meta --json list-intelligence` and check for ADC-relevant kinds (kinds whose description mentions channels, pins, gain, reference, ODR, or compatible strings). Invoke them with `attach-meta --json suggest <kind> [args]` at the right moment:
- A kind listing valid pin names → invoke before channel input assignment (Topic 2)
- A kind listing valid ODR values for a given filter → invoke before Topic 5
- A kind listing compatible string values → invoke to confirm device identity (Topic 1)

Always prefer tool-authored intelligence over the generic guidance here.

---

## Important rules

- Always use `--json` as a global flag: `attach-meta --json <subcommand> ...`
- Always re-read the manifest's command descriptions before invoking `add`, `update`, `read`, or `validate` — never assume path conventions or flag shapes from memory or from this skill
- Read `attach-meta schema responses` before interpreting any response type
- Show human-readable summaries, never raw JSON
- If `ok: false`, show the `message` field and re-prompt before continuing
- Never overwrite an existing value without showing the user what is currently there
- Keep the node name and `reg` in sync: the unit address after `@` must always equal the `reg` value — update both together whenever either changes
