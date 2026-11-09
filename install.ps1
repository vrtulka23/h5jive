# Install or uninstall a native release for the current Windows user.
param(
    [string]$Version = 'latest',
    [string]$InstallDir = $env:H5JIVE_INSTALL_DIR,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\H5Jive'
}
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
if ($InstallDir -eq [IO.Path]::GetPathRoot($InstallDir)) {
    throw 'The installation directory cannot be a drive root.'
}
if ((Test-Path -LiteralPath $InstallDir) -and
    -not (Test-Path -LiteralPath $InstallDir -PathType Container)) {
    throw "Installation path is not a directory: $InstallDir"
}
if (Test-Path -LiteralPath $InstallDir) {
    $item = Get-Item -LiteralPath $InstallDir -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Installation path must not be a symbolic link: $InstallDir"
    }
}

if ($Uninstall) {
    $exe = Join-Path $InstallDir 'h5jive.exe'
    $dip = Join-Path $InstallDir 'settings\DIPfile'
    if (Test-Path -LiteralPath $exe -PathType Leaf) {
        if (-not (Test-Path -LiteralPath $dip -PathType Leaf)) {
            throw "Refusing to remove an unrecognized installation: $InstallDir"
        }
        Remove-Item -LiteralPath $exe -Force
        foreach ($name in @('README.md', 'LICENSE')) {
            $file = Join-Path $InstallDir $name
            if (Test-Path -LiteralPath $file -PathType Leaf) {
                Remove-Item -LiteralPath $file -Force
            }
        }
        Get-ChildItem -LiteralPath $InstallDir -File -Filter '*.dll' |
            Remove-Item -Force
        Write-Host "Uninstalled H5Jive from $InstallDir"
        Write-Host "Settings retained at $(Join-Path $InstallDir 'settings')"
    } else {
        Write-Host 'H5Jive is not installed at the selected location.'
    }
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $remaining = @($userPath -split ';' | Where-Object { $_ -and $_ -ne $InstallDir })
    if ($remaining.Count -ne @($userPath -split ';' | Where-Object { $_ }).Count) {
        [Environment]::SetEnvironmentVariable('Path', ($remaining -join ';'), 'User')
    }
    return
}

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'H5Jive releases require 64-bit Windows.'
}
if ($Version -notmatch '^[A-Za-z0-9._-]+$') {
    throw "Invalid release tag: $Version"
}

$repository = if ($env:H5JIVE_REPOSITORY) { $env:H5JIVE_REPOSITORY } else { 'vrtulka23/h5jive' }
if ($repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
    throw 'H5JIVE_REPOSITORY must be owner/repository.'
}
$base = if ($env:H5JIVE_DOWNLOAD_BASE) {
    $env:H5JIVE_DOWNLOAD_BASE.TrimEnd('/')
} elseif ($Version -eq 'latest') {
    "https://github.com/$repository/releases/latest/download"
} else {
    "https://github.com/$repository/releases/download/$Version"
}

$archive = 'h5jive-windows-x86_64.zip'
$parent = Split-Path -Parent $InstallDir
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$stage = Join-Path $parent ('.h5jive-install.' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null

try {
    $download = Join-Path $stage $archive
    $manifest = Join-Path $stage 'SHA256SUMS'
    Write-Host "Downloading $archive"
    & curl.exe -fsSL "$base/$archive" -o $download
    if ($LASTEXITCODE -ne 0) { throw 'Could not download the release archive.' }
    & curl.exe -fsSL "$base/SHA256SUMS" -o $manifest
    if ($LASTEXITCODE -ne 0) { throw 'Could not download release checksums.' }

    $pattern = '(?m)^([0-9a-fA-F]{64})\s+' + [regex]::Escape($archive) + '\s*$'
    $match = [regex]::Match((Get-Content -LiteralPath $manifest -Raw), $pattern)
    if (-not $match.Success) { throw "Missing checksum for $archive" }
    $actual = (Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash
    if ($actual -ne $match.Groups[1].Value) { throw "Checksum mismatch for $archive" }

    $package = Join-Path $stage 'package'
    Expand-Archive -LiteralPath $download -DestinationPath $package
    $exe = Join-Path $package 'h5jive.exe'
    $dip = Join-Path $package 'settings\DIPfile'
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf) -or
        -not (Test-Path -LiteralPath $dip -PathType Leaf)) {
        throw 'Release archive is missing the executable or DIPL settings.'
    }
    $existingOverride = Join-Path $InstallDir 'settings\overrides.dip'
    if (Test-Path -LiteralPath $existingOverride -PathType Leaf) {
        # Keep this machine's dataset roots when installing a newer release.
        Copy-Item -LiteralPath $existingOverride -Destination (Join-Path $package 'settings\overrides.dip') -Force
    }
    $existingCache = Join-Path $InstallDir 'settings\.cache'
    if (Test-Path -LiteralPath $existingCache -PathType Container) {
        Copy-Item -LiteralPath $existingCache -Destination (Join-Path $package 'settings\.cache') -Recurse -Force
    }
    Push-Location $stage
    try {
        & $exe --inspect | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'The downloaded executable could not read its bundled settings.' }
    } finally {
        Pop-Location
    }

    $backup = Join-Path $stage 'previous'
    if (Test-Path -LiteralPath $InstallDir) {
        Move-Item -LiteralPath $InstallDir -Destination $backup
    }
    try {
        Move-Item -LiteralPath $package -Destination $InstallDir
    } catch {
        if (Test-Path -LiteralPath $backup) {
            Move-Item -LiteralPath $backup -Destination $InstallDir
        }
        throw
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $InstallDir) {
        $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
            $InstallDir
        } else {
            $userPath.TrimEnd(';') + ';' + $InstallDir
        }
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    }
    Write-Host "Installed H5Jive at $InstallDir"
    Write-Host "Executable: $(Join-Path $InstallDir 'h5jive.exe')"
    Write-Host "Dataset paths: $(Join-Path $InstallDir 'settings\overrides.dip')"
    Write-Host 'Open a new terminal to use h5jive.exe from PATH.'
} finally {
    Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
}
