# Builds the mod, wipes the deployed copy, and lays it down fresh.
#
# Wiping rather than overwriting is deliberate: a file you delete from the repo
# would otherwise linger in the game folder forever and keep being loaded.
#
# Run with:  .\deploy.ps1  [-SkipBuild] [-GameDir <path>] [-WhatIf]

[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$GameDir = "D:\SteamLibrary\steamapps\common\Teamfight Manager2",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$repo = $PSScriptRoot
$modId = Split-Path -Leaf $repo

# --- what gets deployed -----------------------------------------------------
# Files and folders, relative to the repo. Folders are copied whole, minus the
# extensions in $ExcludeExtensions below. Everything not named here — src,
# target, docs, previews, tools, Cargo files, .git, .vscode — stays out of the
# game folder.
$Include = @(
    "mod.mod_info",
    "mod.override_info",
    "LICENSE",
    "README.md",
    "$modId.dll",
    "aseprite_resources",
    "ui",
    "text",
    "apply_config.ps1",
    "apply_config.bat",
    "config-default.json",
    "better_mod_menu_profile.json",
    "profile_icon.png",
    "thumbnail.png"
)

# State the mod (or the player) writes in the game folder. A deployed copy is
# kept across the wipe; only when there is none does the repo's copy seed it.
# Overwriting these would throw away in-game item builds and settings.
$UserState = @(
    "mod-settings.json",
    "config.json",
    "4items.cfg"
)

# Same, but never seeded from the repo: the repo's item-builds.json is a working
# copy, and shipping it would hand every player the builds that happen to be in
# it. The mod writes its own on first run.
$PreserveOnly = @(
    "item-builds.json"
)

# Editable source that the engine never reads.
$ExcludeExtensions = @(".xcf", ".psd", ".bak", ".orig")
# ---------------------------------------------------------------------------

$deploy = Join-Path (Join-Path $GameDir "mods") $modId

# The game holds the DLL open, so a wipe would half-finish and leave the mod in
# a state that loads badly. Refuse rather than kill anything.
if (Get-Process -Name "TeamfightManager2" -ErrorAction SilentlyContinue) {
    Write-Error "Teamfight Manager 2 is running - close it first (it holds $modId.dll open)."
}

if (-not $SkipBuild) {
    Write-Host "Building $modId..." -ForegroundColor Cyan
    # From inside the repo, not via -manifest-path: cargo resolves
    # `.cargo/config.toml` and `rust-toolchain.toml` from the *working
    # directory*, and this mod cannot link without either of them.
    Push-Location -LiteralPath $repo
    try {
        cargo build --release
    }
    finally {
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $built = Join-Path $repo "target\release\$modId.dll"
    if (-not (Test-Path -LiteralPath $built)) {
        Write-Error "Build finished but $built is missing."
    }
    Copy-Item -LiteralPath $built -Destination (Join-Path $repo "$modId.dll") -Force
}

# Guard rails before anything is deleted: the target must be a `mods\<mod id>`
# folder and nothing else, so a bad -GameDir cannot wipe a game install.
$parent = Split-Path -Parent $deploy
if ((Split-Path -Leaf $deploy) -ne $modId -or (Split-Path -Leaf $parent) -ne "mods") {
    Write-Error "Refusing to touch '$deploy' - it is not a mods\$modId folder."
}
if (-not (Test-Path -LiteralPath $parent)) {
    Write-Error "No mods folder at '$parent' - is -GameDir right?"
}

# Pull the user state out before the wipe, hold it in memory, put it back after.
$held = @{}
foreach ($name in ($UserState + $PreserveOnly)) {
    $current = Join-Path $deploy $name
    if (Test-Path -LiteralPath $current -PathType Leaf) {
        $held[$name] = [System.IO.File]::ReadAllBytes($current)
    }
}

if (Test-Path -LiteralPath $deploy) {
    if ($PSCmdlet.ShouldProcess($deploy, "Delete contents")) {
        Remove-Item -LiteralPath (Join-Path $deploy "*") -Recurse -Force
    }
}
else {
    New-Item -ItemType Directory -Path $deploy | Out-Null
}

$copied = 0
$skipped = 0
foreach ($name in $Include) {
    $source = Join-Path $repo $name
    if (-not (Test-Path -LiteralPath $source)) {
        Write-Warning "not found, skipping: $name"
        continue
    }

    if (Test-Path -LiteralPath $source -PathType Container) {
        # Rebuild the tree by hand so the extension filter applies at any depth.
        foreach ($file in Get-ChildItem -LiteralPath $source -Recurse -File) {
            if ($ExcludeExtensions -contains $file.Extension.ToLower()) {
                $skipped++
                continue
            }
            $relative = $file.FullName.Substring($repo.Length).TrimStart('\')
            $destination = Join-Path $deploy $relative
            $destinationDir = Split-Path -Parent $destination
            if (-not (Test-Path -LiteralPath $destinationDir)) {
                New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null
            }
            Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
            $copied++
        }
    }
    else {
        Copy-Item -LiteralPath $source -Destination (Join-Path $deploy $name) -Force
        $copied++
    }
}

$restored = 0
$seeded = 0
foreach ($name in ($UserState + $PreserveOnly)) {
    $destination = Join-Path $deploy $name
    if ($held.ContainsKey($name)) {
        [System.IO.File]::WriteAllBytes($destination, $held[$name])
        $restored++
        continue
    }
    if ($PreserveOnly -contains $name) { continue }
    $source = Join-Path $repo $name
    if (Test-Path -LiteralPath $source -PathType Leaf) {
        Copy-Item -LiteralPath $source -Destination $destination -Force
        $seeded++
    }
}

Write-Host "Deployed $copied files to $deploy" -ForegroundColor Green
if ($skipped) { Write-Host "Skipped $skipped source files ($($ExcludeExtensions -join ', '))" }
if ($restored) { Write-Host "Kept $restored existing user-state file(s): $(($UserState + $PreserveOnly) -join ', ')" }
if ($seeded) { Write-Host "Seeded $seeded user-state file(s) from the repo" }
