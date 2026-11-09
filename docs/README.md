# H5Jive documentation

The project guide is written with [mdBook](https://rust-lang.github.io/mdBook/).
It covers installation, data exploration, settings, and the internal layout of
the Rust workspace. Rust API documentation is generated separately by
`cargo doc --workspace --open`.

Build the guide with `mdbook build docs` and open `docs/book/index.html`.
Install mdBook with `cargo install mdbook` if it is not already available.

## Sections

- [Getting started](src/getting-started.md)
- [Exploring data](src/exploring-data.md)
- [Settings and data formats](src/settings-and-formats.md)
- [Architecture](src/development.md)
- [Troubleshooting](src/troubleshooting.md)
