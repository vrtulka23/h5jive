# Getting started

## Install a native release

The [release archives](https://github.com/vrtulka23/h5jive/releases)
contain the executable and a runtime `settings/` directory. The installers
verify the archive checksum and place the files in a user-owned directory.
They preserve `settings/overrides.dip` when upgrading.

The macOS DMGs are currently ad hoc signed and not Apple notarized. If macOS
shows “Apple could not verify H5Jive is free of malware,” confirm that the DMG
came from the [official release](https://github.com/vrtulka23/h5jive/releases/latest).
After trying to open the app once, go to **System Settings → Privacy & Security**
and select **Open Anyway**. This allows this app without disabling Gatekeeper
system-wide. See [Apple's instructions](https://support.apple.com/en-us/102445).

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
graphics backend to run the app. On Ubuntu 22.04, install build dependencies with:

```sh
sudo apt-get update
sudo apt-get install -y build-essential ca-certificates clang cmake curl git \
  pkg-config libhdf5-dev libclang-dev libxcb1-dev libxrandr-dev \
  libdbus-1-dev libx11-dev libwayland-dev libegl-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libgl1-mesa-dev libvulkan-dev
```

Prebuilt Linux releases bundle HDF5 and its compression helpers. They still
need a compatible Linux graphics stack on the host.

### Linux graphics on SUSE

The Linux archive is built on Ubuntu and intended for openSUSE Leap. Its
bundled HDF5 is built without the unused S3 driver, so it does not require
Ubuntu's versioned libcurl symbols from the host. This applies to releases
built with the updated workflow; an already installed release is unchanged
until it is upgraded.

If startup reports `AdapterUnavailable`, the executable has started but no
usable graphics adapter was found for the current display. In a local desktop
session, install or enable your distribution's Vulkan or OpenGL driver. You can
try Vulkan with an existing installation using `WGPU_BACKEND=vulkan h5jive`.
For a forwarded X11 display, you can try both
backends by adding this line to
`~/.local/share/h5jive/settings/overrides.dip`:

```text
viewer.graphics.forwarded_x11_backend = "vulkan,gl"
```

If the display exposes neither backend, the window cannot render through that
session; a graphical login or a remote desktop with graphics support is needed.
With automatic graphics selection on Linux, H5Jive tries both backends with the
window surface. If graphics initialization fails, it restarts once with OpenGL
alone. An explicit `WGPU_BACKEND` or `viewer.graphics.backend` setting is
respected and does not trigger this retry.

### File drops on Linux

The winit 0.30 Wayland backend does not deliver file drag-and-drop events to
H5Jive. When an X11 display is available, H5Jive automatically selects X11;
on a Wayland desktop this uses XWayland if the compositor provides it. This
applies to Ubuntu 22.04 and openSUSE Leap as well.
Start it normally with `h5jive`; no display variables need to be changed in
the shell. If X11 is unavailable, H5Jive uses native Wayland and file drops
remain unavailable; use **Browse A** or **Browse B**. To request a native
Wayland window even when XWayland is available, run:

```sh
H5JIVE_WINDOW_BACKEND=wayland h5jive
```

On X11, H5Jive reads the pointer position relative to its window while a file
is hovered, then uses the same position to choose A or B on drop. Winit 0.30
does not expose XDND drag-motion coordinates, so a compositor that does not
update the X11 pointer during a Wayland-to-XWayland drag may still leave the
highlight stationary. If no position is available, H5Jive leaves both drop
regions unselected and asks you to use Browse A or Browse B.

### Run over SSH with X11 forwarding

The Linux app can display its window through SSH X11 forwarding. Connect from a
machine running an X server, then launch H5Jive on the remote host:

```sh
ssh -Y user@host
echo "$DISPLAY"
h5jive
```

The remote `DISPLAY` must be set, and the X server must expose a usable
graphics adapter for the window to render. With the default
`viewer.graphics.backend = "auto"`, H5Jive detects a forwarded X11 display
and tries Vulkan and OpenGL. This is set by
`viewer.graphics.forwarded_x11_backend = "vulkan,gl"` in DIPL. If neither
backend can create an adapter for the forwarded display, run H5Jive in a local
desktop session on the Linux machine or use a remote desktop with graphics
support. You can also run
`./h5jive.sh --run` from a source checkout after building it. Building alone
does not require an X11 session.

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
You can also drag one local file at a time over the H5Jive window. Two drop panels appear:
drop on the left for File A or on the right for File B. If File A is empty,
a file dropped on B waits until A is loaded. Native file drops are supported
on macOS, Windows, and Linux X11. The current window library does not deliver
file-drop events on Wayland; use **Files** → **Browse** there.
For an H5Jive window forwarded over SSH, the dropped path must exist on the
remote machine; dragging a file that exists only on your local computer does
not transfer its contents.

## Try the examples

The bundled `examples/arrays/space_demo.hdf5` contains 1D–4D datasets.
The bundled `examples/satellite/gome2_band3_20130106_gome.nc` contains a
sample extracted from a real GOME-2 Level 1B product. Select
`MODE_NADIR/BAND_3/OBSERVATIONS/radiance` and open the world map or globe to
inspect its geolocated pixels. The sample retains all 625 scan lines, 32 ground
pixels, four-corner bounds, scan direction, and 64 radiance channels. See its
[data attribution and licence](../../examples/satellite/LICENSE.md).
