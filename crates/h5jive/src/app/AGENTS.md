# App conventions

This directory owns the native Dear ImGui UI, view controls, rendering,
settings, exports, and background job coordination. `../app.rs` declares the
application state and wires these modules together. Read `docs/src/development.md`
before changing a cross-cutting flow.

1. Keep rendering and interaction in `views/`, `details/`, `layout.rs`, or
   focused UI helpers. Put GUI-independent data calculations and slice models
   in `../core/`. Reuse `h5jive-data` for file access. Plugins belong in
   `../plugins/`, with their own scoped `AGENTS.md`.
2. Keep source and dataset identity explicit as `(file slot, dataset path)`.
   Files A and B come from the same `App.files` slots throughout the UI.
   When a file or selection changes, invalidate dependent caches and jobs;
   follow the existing reset and generation patterns in `layout.rs` and
   `jobs.rs`.
3. Do costly file reads and refinements outside the frame loop. Existing jobs
   reopen files on worker threads, send results through channels, and use
   generation counters to discard stale work. Keep each frame responsive.
4. Use `Settings` for behavior and sizes that users should configure, and
   update the matching DIPL schema and source file in `settings/`. Use palette
   colors and the shared UI helpers rather than hardcoded theme colors.
5. Preserve Dear ImGui scope pairing and stable widget IDs. Use a bounded
   `ListClipper` or table scrolling for large lists. Keep view and plugin
   selections usable when files are absent or loading.
6. Run `cargo fmt --all`, `cargo check -p h5jive`, and relevant tests. Run the
   full suite with `cargo test -p h5jive -- --test-threads=1` because DIPL and
   Dear ImGui tests share process state.
