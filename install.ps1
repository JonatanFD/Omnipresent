# Installer for omni (omnipresent) on Windows.
#
#   powershell -c "irm https://github.com/JonatanFD/omnipresent/releases/latest/download/install.ps1 | iex"
#
# Downloads the prebuilt omni.exe for this machine from the latest GitHub
# release and installs it. No Rust toolchain or C compiler required.

$ErrorActionPreference = 'Stop'

$repo = 'JonatanFD/omnipresent'

# Match the build to the machine. PROCESSOR_ARCHITECTURE reports the *process*
# architecture, so an x86 PowerShell on an Arm box would answer AMD64 — the
# native value in PROCESSOR_ARCHITEW6432 wins when it is set.
$arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$target = switch ($arch) {
    'ARM64' { 'aarch64-pc-windows-msvc' }
    'AMD64' { 'x86_64-pc-windows-msvc' }
    default { throw "unsupported Windows architecture: $arch" }
}
$asset = "omni-$target.zip"
$url = "https://github.com/$repo/releases/latest/download/$asset"

# Per-user install location (no admin rights needed).
$installDir = if ($env:OMNI_INSTALL_DIR) {
    $env:OMNI_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\omni'
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("omni-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
try {
    Write-Host "downloading $asset ..."
    $zip = Join-Path $tmp $asset
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath $tmp -Force

    $exe = Join-Path $tmp 'omni.exe'
    if (-not (Test-Path $exe)) {
        throw "the archive did not contain omni.exe"
    }

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -Path $exe -Destination (Join-Path $installDir 'omni.exe') -Force
    Write-Host "installed omni to $installDir\omni.exe"

    # Add the install dir to the user PATH if it is not already there.
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $installDir) {
        $newPath = if ([string]::IsNullOrEmpty($userPath)) { $installDir } else { "$userPath;$installDir" }
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        Write-Host ""
        Write-Host "added $installDir to your user PATH — open a new terminal for it to take effect."
    }

    Write-Host ""
    Write-Host "done. Try:  omni --help"
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
