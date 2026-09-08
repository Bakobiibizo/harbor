# Harbor Neo Grounds WASM runtime

Source inputs:

- Neo Grounds runtime contracts at `hydra-dynamix/neo-grounds@4959256`
- Harbor local package/library contract at commit `bdfbe3e`
- shared `fixtures/harbor-game/v1/minimal-game.wasm`

## Isolation and lifecycle

Harbor loads package bytes only from its profile-managed immutable library after Rust revalidation. The frontend sends the validated bundle to a dedicated module Worker. The Worker receives no Tauri object, filesystem handle, credential, contact data, or network binding. Generated WASM receives only the fixed `neo_grounds` function imports listed below; unknown modules, memories, globals, tables, and functions fail before instantiation.

The required exports are `ng_init`, `ng_update`, `ng_render`, and `ng_shutdown`. Start calls initialization and starts a bounded frame loop. Stop calls shutdown and retains the validated module for an explicit restart. Navigation/destroy sends stop, removes every listener and timer, terminates the Worker, closes audio, and detaches input.

The shared real fixture compiles and executes all lifecycle exports. Its render lifecycle produces the host's empty-frame clear fallback, proving the Worker-to-Canvas path even when a minimal game emits no draw operations. Installed bundles relaunch without any store or network fetch.

## Host ABI

Allowed module: `neo_grounds`

- `render_clear(r, g, b, a)`
- `render_fill_rect(x, y, width, height, r, g, b, a)`
- `audio_tone(frequency, duration_ms, volume)`
- `input_key_down(fnv1a_code)`
- `input_pointer_down()`
- `input_pointer_x()` / `input_pointer_y()`
- `viewport_width()` / `viewport_height()`

Canvas, WebAudio, keyboard/pointer focus, resize, expand, and fullscreen remain host-owned. Render operations cross a closed typed command buffer and are validated again on the main thread. Audio accepts only 20–20,000 Hz, 1–5,000 ms, and volume 0–1. Multiplayer fails before launch. In this runtime version, `save_data` is the only accepted optional manifest capability; durable saves cross the separately bounded profile service rather than becoming a general WASM/browser import.

## Measured limits

A Node runtime measurement on 2026-09-08 executed 100,000 update/render pairs from the 91-byte shared fixture in 3.112 ms total: 0.000031 ms per pair. The fixture exports no linear memory and emits one host fallback clear per frame.

- WASM bytes: 8 MiB, inherited from the validated package bound and over 92,000 times the measured 91-byte fixture. Protects compilation memory and startup work.
- Linear memory: 64 MiB. The current fixture measures zero bytes; the absolute 64 MiB safety allowance supports generated state while bounding Worker memory growth. Memory is checked after initialization and every frame.
- Render commands: 4,096 per frame, 4,096 times the measured fallback command. Protects structured-clone and Canvas main-thread work.
- Host calls: 8,192 per frame; the current fixture measures zero. This explicit allowance supports command-heavy generated games while bounding import-call abuse.
- Slow frame: 50 ms, over 1.6 million times the measured lifecycle pair. Five consecutive violations terminate the runtime, protecting sustained responsiveness without treating one scheduling pause as a game defect.
- Watchdog: 2,000 ms without loading progress or a running heartbeat. Protects against infinite WASM loops that cannot be preempted inside a Worker call.
- Canvas dimension: 8,192 pixels per axis; command coordinates are limited to ±32,768. Protects Canvas allocation and pathological raster operations.
- Save slot: 1 MiB, enforced by Rust and documented in the local-library contract.

A trap, invalid command, repeated slow frame, memory excess, Worker crash, or watchdog timeout produces an actionable error state and terminates the Worker. Limits must not be raised without new measured build/runtime evidence and a recorded protected failure mode.
