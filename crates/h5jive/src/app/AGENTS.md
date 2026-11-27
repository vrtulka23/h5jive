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
   update the matching DIPL schema and source file in `settings/runtime/`.
   Build-only constants live in `settings/build/`. Use palette
   colors and the shared UI helpers rather than hardcoded theme colors.
5. Preserve Dear ImGui scope pairing and stable widget IDs. Use a bounded
   `ListClipper` or table scrolling for large lists. Keep view and plugin
   selections usable when files are absent or loading.
   Put general explanations for the active section in the top Help menu's
   "Help for current section" popup, not among controls or inside the viewer.
   Add a topic in `layout.rs` when adding an inspector view or workspace section.
   Give new interactive controls a short Tutorial tip through `item_tip` in
   `tutorial_tips.rs`. Preserve the normal tooltip when Tutorial tips are off; use an
   empty normal text if the control has no existing tooltip. Keep tips focused
   on the action the user can take. Use `item_plain_tip` or `show_plain_tip`
   for other tooltips so their text has the same padding.
   Add transient application state notices with `App::show_notice` or the
   shared `NoticeCenter` and route all of them through the top-center status
   stack in `layout.rs` and `draw_notice_bubble` in `ui.rs`. This covers file
   opens, dataset scans and metadata, exports, searches, edits, view jobs, and
   plugin jobs. Derive loading text from active work; do not render separate
   loading, progress, or job-completion text inline. Emit an Error notice when
   background work fails or a worker disconnects, even when a persistent panel
   error remains as an actionable fallback. Loading bubbles appear only while
   work is active. Info bubbles (neutral status) last 5 seconds, Success bubbles
   (completed operations) last 4 seconds, and Warning bubbles (recoverable
   problems) last 10 seconds. Error bubbles stay until their `[x]` control is
   clicked or the app programmatically dismisses them when their condition is
   resolved. Use the palette notice colors, not hardcoded colors, for blue
   loading/info, green success, amber warnings, and red errors. Do not add
   another notice field, toast, or overlay. Keep dialog/form validation,
   file-picker validation, and
   persistent actionable empty/error states inline. Figure-local render-quality
   badges such as Preview, Settled, Updating, Overview, Refining, or resolution
   details belong to the visualization itself; keep them only when they
   describe the displayed data rather than duplicating app-wide work. Loading
   and notices stack through this same mechanism, oldest first so new bubbles
   append at the bottom; bubbles must remain visible and dismissible above
   modal overlays.
6. Run `cargo fmt --all`, `cargo check -p h5jive`, and relevant tests. Run the
   full suite with `cargo test -p h5jive -- --test-threads=1` because DIPL and
   Dear ImGui tests share process state.
