# Getting started

## Install a native release

Once published, the [release archives](https://github.com/vrtulka23/h5jive/releases)
contain the executable and a runtime `settings/` directory. The installers
verify the archive checksum and place the files in a user-owned directory.
They preserve `settings/overrides.dip` when upgrading.

On macOS or Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.sh | sh
```

On Windows, run these commands in PowerShell:

```powershell
curl.exe -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.ps1 -o "$env:TEMP\h5jive-install.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\h5jive-install.ps1"
```

The Unix command puts a launcher in `~/.local/bin`; add that directory to
`PATH` if your shell does not already include it. The Windows installer adds
its directory to the user `PATH`; open a new terminal to pick up the change.
No administrator permission is needed for either installer. A specific release
can be selected with `sh install.sh --version vX.Y.Z` or
`install.ps1 -Version vX.Y.Z` after downloading the installer script.

Uninstall on macOS or Linux with:

```sh
curl -fsSL https://raw.githubusercontent.com/vrtulka23/h5jive/main/install.sh | sh -s -- --uninstall
```

On Windows, download `install.ps1` as above and run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\h5jive-install.ps1" -Uninstall
```

Uninstall removes the program and bundled libraries, removes the Windows user
`PATH` entry, and retains `settings/` for a later reinstall.

The bundled DIPL project is read every time the application starts. Edit
`settings/overrides.dip` in the installation directory to set local dataset
roots. The [settings guide](settings-and-formats.md) explains how to register
additional sources and formats.

## Requirements

Builds need Rust and Cargo, CMake, a C++ compiler, and the HDF5 C library. The
SNT3 0.10.2 Rust binding builds its matching native source by default. On macOS,
the launch script uses the system graphics backend; Linux also needs a working
graphics session and adapter. On Ubuntu 22.04, install build dependencies with:

```sh
sudo apt-get update
sudo apt-get install -y build-essential ca-certificates clang cmake curl git \
  pkg-config libhdf5-dev libclang-dev libxcb1-dev libxrandr-dev \
  libdbus-1-dev libx11-dev libwayland-dev libegl-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libgl1-mesa-dev libvulkan-dev
```

Prebuilt Linux releases bundle HDF5 and its compression helpers. They still
need a compatible Linux graphics stack on the host.

## Build and launch

From the repository root:

```sh
./h5jive.sh --build
./h5jive.sh --run
./h5jive.sh --build --run path/to/data.h5 path/to/other.nc
./h5jive.sh --run path/to/data.nc:/group/radiance
```

Pass file paths to open them at launch. Append `:/dataset/path` to select a
dataset immediately; with two files, the first specified dataset is selected.
`./h5jive.sh --help` lists the
available options. `SNT3_PREFIX` and `SNT3_BUILD_DIR` can select an external
native SNT3 build; otherwise the crate's bundled SNT3 source is used.
You can also browse the system folders and set or replace input file A and B
from the **Files** menu while the viewer is running; both file trees remain
available in the sidebar.

## Try the examples

The array demo generator writes `examples/arrays/space_demo.hdf5` with 1D–4D
datasets. The bundled `examples/satellite/gome2_band3_20130106_gome.nc` contains a
sample extracted from a real GOME-2 Level 1B product. Select
`MODE_NADIR/BAND_3/OBSERVATIONS/radiance` and open the world map or globe to
inspect its geolocated pixels. The sample retains all 625 scan lines, 32 ground
pixels, four-corner bounds, scan direction, and 64 radiance channels.
