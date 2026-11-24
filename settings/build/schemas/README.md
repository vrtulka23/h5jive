# Build setting schemas

These DIPL schemas define the types, defaults, and bounds of compiled plugin
layout and resource limits. The parent [build project](../README.md) registers
them in `DIPfile`, then applies values from its sibling `.dip` files. Rust code
consumes the validated result through generated constants.
