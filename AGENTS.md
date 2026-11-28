# Repository conventions

H5Jive is a two-crate workspace with deliberately narrow dependency
directions. Preserve them when adding code.

1. `crates/data-source` (`h5jive-data`) is the only crate allowed to depend on
   HDF5, NetCDF, or ndarray backends directly. It owns file opening, dataset
   discovery, reads, mutations, copies, statistics, and export. Format-specific
   logic belongs here; expose path-based, format-neutral operations to callers.
2. `crates/h5jive/src/core/` is GUI-independent analysis and indexing logic.
   It may use `h5jive-data`; it must not depend on `app`, `plugins`, or UI
   libraries. Scanning and loading routines accept cancellation and progress
   callbacks instead of exposing channels.
3. `crates/h5jive/src/app/` owns the UI, view state, settings, and job polling.
   `crates/h5jive/src/plugins/` owns Plugin Lab tools. Neither may import
   HDF5, NetCDF, or ndarray APIs in production code. Tests may use the
   `hdf5-metno` dev-dependency for fixtures and verification.
4. Long-running app and plugin work uses `BackgroundJob` or `ProgressJob` from
   `app/background_job.rs`. Do not add direct `mpsc` receivers, `try_recv`
   loops, or ad-hoc worker polling outside that module. Surface every job
   failure through the shared notice stack. Core code should accept callbacks
   so plugins can wrap it in the same job types.
5. Transient status uses `NoticeCenter`/`App::show_notice` and the top-center
   bubble stack. Follow `crates/h5jive/src/app/AGENTS.md` and
   `crates/h5jive/src/plugins/AGENTS.md` for notice semantics.
6. Run `./scripts/check-layering.sh` after structural changes. The full
   validation sequence is `cargo fmt --all`, `cargo check --workspace
   --all-targets`, `cargo test -p h5jive-data -- --test-threads=1`, and
   `cargo test -p h5jive -- --test-threads=1`.

Read `docs/src/development.md` before changing a cross-cutting flow. Deferred
until a concrete need appears: extracting `h5jive-core`, a separate config
crate, merging core into `h5jive-data`, and per-plugin crates.
