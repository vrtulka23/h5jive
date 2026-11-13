# H5Jive

**Dig into your data.**

[![Build and test](https://github.com/vrtulka23/h5jive/actions/workflows/build-test.yml/badge.svg)](https://github.com/vrtulka23/h5jive/actions/workflows/build-test.yml)
[![Native release builds](https://github.com/vrtulka23/h5jive/actions/workflows/native-release.yml/badge.svg)](https://github.com/vrtulka23/h5jive/actions/workflows/native-release.yml)
[![Latest release](https://img.shields.io/github/v/release/vrtulka23/h5jive)](https://github.com/vrtulka23/h5jive/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Guide](https://img.shields.io/badge/docs-project%20guide-orange.svg)](docs/README.md)

<img src="website/public/h5jive.png" alt="H5Jive layered data logo" width="120" align="right" />

H5Jive is a native desktop explorer for HDF5 and NetCDF data. Browse local
products, inspect dataset metadata and statistics, and view tables, profiles,
2D slices, 3D volumes, and geolocated satellite swaths on a map or globe.

## Install a release

Once a release with native assets is published, the installers download the
archive for your system, verify its SHA-256 checksum, and install it in your
user directory. No administrator access is needed. The macOS and Linux
installer requires `curl` and `tar`.

**macOS or Linux:**

```sh
curl -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.sh | sh
```

**Windows PowerShell:**

```powershell
curl.exe -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.ps1 -o "$env:TEMP\h5jive-install.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\h5jive-install.ps1"
```

The Unix launcher is placed at `~/.local/bin/h5jive`; add that directory
to `PATH` if needed. Windows installs to `%LOCALAPPDATA%\Programs\H5Jive`
and adds that directory to the user `PATH`. Open a new terminal after installing.
To install a specific release, pass `--version vX.Y.Z` to the Unix script or
`-Version vX.Y.Z` to the PowerShell script.

To uninstall on macOS or Linux, run:

```sh
curl -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.sh | sh -s -- --uninstall
```

On Windows, run:

```powershell
curl.exe -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.ps1 -o "$env:TEMP\h5jive-install.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\h5jive-install.ps1" -Uninstall
```

Uninstall removes the executable and its bundled libraries; it retains
`settings/` so local dataset paths are available on reinstall.

Each release archive also contains the executable and a `settings/` directory
for manual installation. DIPL reads those settings at runtime. Edit
`settings/overrides.dip` to point registered datasets at directories on your
machine; the installer preserves this file during upgrades. For example:

```text
dataset_sources[mock_gome].root = "/data/GOME"
dataset_sources[mock_gome2].root = "/data/GOME2"
```

Use forward slashes in Windows paths, such as `D:/satellite/GOME`. Dataset
sources with missing roots remain visible and are marked unavailable.
See [settings and data formats](docs/src/settings-and-formats.md) for adding
sources, formats, tags, and product discovery rules.

## Run

Launch `h5jive` (or `h5jive.exe` on Windows). Choose files from the
**Files** menu or pass one or two paths on the command line:

```sh
h5jive path/to/product_A.h5 path/to/product_B.nc
h5jive path/to/product.nc:/group/radiance
```

Append `:/dataset/path` to a file path to select that dataset when the viewer
opens. With two files, the first specified dataset is selected initially.

The top mode bar offers a dataset browser, a data inspector, and a comparison
workspace. The [exploring data guide](docs/src/exploring-data.md) explains the
available views and controls.

## Build from source

Install Rust/Cargo 1.95 or newer, CMake, a C/C++ toolchain, and the HDF5 C
library. On Linux, also install the graphics development libraries listed in
the [getting started guide](docs/src/getting-started.md).

```sh
./h5jive.sh --build
./h5jive.sh --run
./h5jive.sh --run examples/gome2_band3_20130106_gome.nc:/MODE_NADIR/BAND_3/OBSERVATIONS/radiance
target/release/h5jive --view-globe examples/gome2_band3_20130106_gome.nc:/MODE_NADIR/BAND_3/OBSERVATIONS/radiance
```

Both the script and executable accept `--view-data`, `--view-1d-profiles`,
`--view-2d-slice`, `--view-3d`, `--view-world-map`, and `--view-globe` when a
`FILE:/DATASET` selection is supplied.

The repository includes a deterministic [HDF5 demo](examples/space_demo.hdf5)
and a [GOME-2 Band 3 sample](examples/gome2_band3_20130106_gome.nc) extracted
from a real Level 1B product. You can inspect a file tree
without opening a window:

```sh
cargo run -p h5jive -- --inspect examples/space_demo.hdf5
```

## Documentation

- [Getting started](docs/src/getting-started.md): build, launch, and examples
- [Exploring data](docs/src/exploring-data.md): views and controls
- [View contracts](contracts/README.md): expected behavior and regression sequences for every view
- [Settings and data formats](docs/src/settings-and-formats.md): DIPL, dataset sources, and format discovery
- [Architecture](docs/src/development.md): crates, core logic, UI, and performance checks
- [Troubleshooting](docs/src/troubleshooting.md): common build and data issues

The guide is built with mdBook; see [docs/README.md](docs/README.md). The
[project page](website/README.md) and guide are deployed together by the
[Pages workflow](.github/workflows/pages.yml). Publishing a
GitHub release or running the [native release workflow](.github/workflows/native-release.yml)
for an existing tag creates the platform archives and checksum manifest.

H5Jive is maintained by Ondrej Pêgo Jaura and released under the
[MIT license](LICENSE).
