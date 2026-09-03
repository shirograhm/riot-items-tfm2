<#
.SYNOPSIS
  Reapply custom and modified items from sideloader/ after merging upstream.
  Run this after fetching and merging shiro/<branch>, before building.
  Existing entries are updated, so you can run it again without adding duplicates.

  To change the items, edit with sideload-editor.exe.
  Advanced users may edit sideloader/manifest.json or sideloader/items/*.rs directly
  and run this again. Changes made directly inside the generated SIDELOADER
  blocks will be overwritten.
#>

$ErrorActionPreference = 'Stop'

# Allow the script to run from either sideloader/ or a copy deployed
# straight to the mod root (e.g. for a PR upstream).
if ((Split-Path -Leaf $PSScriptRoot) -eq 'sideloader') {
    $root = Split-Path -Parent $PSScriptRoot
    $sider = $PSScriptRoot
}
else {
    $root = $PSScriptRoot
    $sider = Join-Path $PSScriptRoot 'sideloader'
}

function Read-Utf8($path) {
    # Read as UTF-8 explicitly so characters like em dashes don't get mangled
    # by the default encoding on older PowerShell versions.
    [System.IO.File]::ReadAllText($path, (New-Object System.Text.UTF8Encoding($false)))
}

function Write-Utf8NoBom($path, [string]$content) {
    [System.IO.File]::WriteAllText($path, $content, (New-Object System.Text.UTF8Encoding($false)))
}

function Get-Newline([string]$text) {
    # Keep the file's existing line endings to avoid noisy diffs.
    if ($text -match "`r`n") { return "`r`n" }
    return "`n"
}

$manifest = Read-Utf8 (Join-Path $sider 'manifest.json') | ConvertFrom-Json
$items = $manifest.items.PSObject.Properties | ForEach-Object { $_.Value | Add-Member -NotePropertyName slug -NotePropertyValue $_.Name -PassThru }

# Copy item sources
# The manifest's dir field points to the item's category folder in src/items/.
foreach ($item in $items) {
    if (-not $item.module -or -not $item.dir) { continue }
    $src = Join-Path $sider "items\$($item.module).rs"
    $dst = Join-Path $root "src\items\$($item.dir)\$($item.module).rs"
    Copy-Item $src $dst -Force
    Write-Host "[items] $($item.module).rs <- sideloader"
}

# Add missing module declarations
# Only items marked declare_in_dir_mod need this. Modified upstream items
# like deaths_dance already have a declaration.
$dirModCache = @{}
foreach ($item in $items) {
    if (-not $item.module -or -not $item.dir -or -not $item.declare_in_dir_mod) { continue }
    $dirModPath = Join-Path $root "src\items\$($item.dir)\mod.rs"
    if (-not $dirModCache.ContainsKey($dirModPath)) {
        $dirModCache[$dirModPath] = Read-Utf8 $dirModPath
    }
    $dirMod = $dirModCache[$dirModPath]
    if ($dirMod -notmatch "(?m)^\s*$([regex]::Escape($item.module)),\s*$") {
        $nl = Get-Newline $dirMod
        $dirModCache[$dirModPath] = $dirMod -replace '(?s)(items!\s*\{)', "`$1$nl    $($item.module),"
        Write-Host "[items/$($item.dir)/mod.rs] added module $($item.module)"
    }
}
foreach ($path in $dirModCache.Keys) { Write-Utf8NoBom $path $dirModCache[$path] }

# Update the tier 4 and tier 5 registrations in src/lib.rs
$libRsPath = Join-Path $root 'src\lib.rs'
$libRs = Read-Utf8 $libRsPath
$libNl = Get-Newline $libRs

function Set-MarkerBlock([string]$text, [string]$nl, [string]$tag, [string]$anchorRegex, [string[]]$lines) {
    # Give each block its own tag so replacing tier 4 doesn't remove tier 5.
    $beginMark = "    // >>> SIDELOADER $tag BEGIN (managed by sideloader/sideloader.ps1 - do not hand-edit) <<<"
    $endMark = "    // <<< SIDELOADER $tag END >>>"
    # Remove the extra newline from the last run too, or blank lines pile up.
    $blockRegex = '\r?\n' + [regex]::Escape($beginMark) + '[\s\S]*?' + [regex]::Escape($endMark) + "`r?`n?"
    $text = [regex]::Replace($text, $blockRegex, '')
    $body = ($lines | ForEach-Object { "    $_" }) -join $nl
    $block = "$beginMark$nl$body$nl$endMark$nl"
    return [regex]::Replace($text, $anchorRegex, { param($m) $m.Value + $nl + $block }, 1)
}

$tier4Lines = @()
$tier5Lines = @()
foreach ($item in $items) {
    if (-not $item.lib_registrations -and -not $item.lib_radiant_registrations) { continue }
    foreach ($reg in $item.lib_registrations) {
        if ($item.enabled -eq $false) {
            $tier4Lines += "// Temporarily disabled: $($item.slug) ($($item.disabled_reason))"
            $tier4Lines += "// $reg"
        }
        else {
            $tier4Lines += $reg
        }
    }
    foreach ($reg in $item.lib_radiant_registrations) {
        if ($item.enabled -eq $false) {
            $tier5Lines += "// Temporarily disabled: radiant_$($item.slug)"
            $tier5Lines += "// $reg"
        }
        else {
            $tier5Lines += $reg
        }
    }
}

if ($tier4Lines.Count -gt 0) {
    # Keep the line ending out of the match. Using \s* here would also consume
    # the \r in CRLF files and split the line ending when the block is inserted.
    $libRs = Set-MarkerBlock $libRs $libNl 'TIER4' '(?m)^\s*//\s*Tier 4[ \t]*(?=\r?\n|$)' $tier4Lines
}
if ($tier5Lines.Count -gt 0) {
    $libRs = Set-MarkerBlock $libRs $libNl 'TIER5' '(?m)^\s*//\s*Tier 5[ \t]*(?=\r?\n|$)' $tier5Lines
}
Write-Utf8NoBom $libRsPath $libRs
Write-Host "[lib.rs] rewrote SIDELOADER blocks ($($tier4Lines.Count) tier-4 / $($tier5Lines.Count) tier-5 lines)"

# Update CATEGORY_OF in src/item_catalog.rs and keep it sorted
$catalogPath = Join-Path $root 'src\item_catalog.rs'
$catalog = Read-Utf8 $catalogPath
$catalogNl = Get-Newline $catalog
$catalogMatch = [regex]::Match($catalog, '(?s)(const CATEGORY_OF: &\[\(&str, &str\)\] = &\[)(.*?)(\r?\n\];)')
if ($catalogMatch.Success) {
    $prefix, $body = $catalogMatch.Groups[1].Value, $catalogMatch.Groups[2].Value
    $entries = [ordered]@{}
    foreach ($m in [regex]::Matches($body, '\("([^"]+)",\s*"([^"]+)"\)')) {
        $entries[$m.Groups[1].Value] = $m.Groups[2].Value
    }
    foreach ($item in $items) {
        if ($item.category) { $entries[$item.slug] = $item.category }
    }
    $sortedKeys = $entries.Keys | Sort-Object
    $newBody = ($sortedKeys | ForEach-Object { "    (`"$_`", `"$($entries[$_])`")," + $catalogNl }) -join ''
    $newBlock = "$prefix$catalogNl$newBody];"
    $catalog = $catalog.Substring(0, $catalogMatch.Index) + $newBlock + $catalog.Substring($catalogMatch.Index + $catalogMatch.Length)
    Write-Utf8NoBom $catalogPath $catalog
    Write-Host "[item_catalog.rs] CATEGORY_OF has $($sortedKeys.Count) entries"
}
else {
    Write-Warning "[item_catalog.rs] could not find CATEGORY_OF array - skipped"
}

# Add the extra ItemConfig fields and optional JSON BOM fix
$configRsPath = Join-Path $root 'src\config.rs'
$configRs = Read-Utf8 $configRsPath
$configNl = Get-Newline $configRs
# Keep this as an array, even when there's only one extra field.
$fieldLines = @($manifest.config_rs_extra_fields | ForEach-Object { "    pub $($_.name): $($_.type)," })
$beginMark = '    // >>> SIDELOADER BEGIN (managed by sideloader/sideloader.ps1 - do not hand-edit) <<<'
$endMark = '    // <<< SIDELOADER END >>>'
# Keep the newline before this block: it belongs to the struct's opening
# brace. Unlike Set-MarkerBlock, there's no extra separator to remove.
$blockRegex = [regex]::Escape($beginMark) + '[\s\S]*?' + [regex]::Escape($endMark) + "`r?`n?"
$configRs = [regex]::Replace($configRs, $blockRegex, '')
if ($fieldLines.Count -gt 0) {
    $block = "$beginMark$configNl" + ($fieldLines -join $configNl) + "$configNl$endMark$configNl"
    $configRs = [regex]::Replace($configRs, '(?m)^(pub struct ItemConfig \{\s*\r?\n)', { param($m) $m.Value + $block }, 1)
}
if ($manifest.config_rs_bom_fix -and $configRs -notmatch [regex]::Escape("strip_prefix('\u{feff}')")) {
    $bomFix = ".and_then(|s| {${configNl}            let json = s.strip_prefix('\u{feff}').unwrap_or(&s);${configNl}            serde_json::from_str(json).ok()${configNl}        })"
    $configRs = [regex]::Replace($configRs, [regex]::Escape('.and_then(|s| serde_json::from_str(&s).ok())'), { param($m) $bomFix }, 1)
}
Write-Utf8NoBom $configRsPath $configRs
Write-Host "[config.rs] $($fieldLines.Count) extra fields, BOM fix applied"

# Update item defaults in config-default.json
# Replace entries as text so PowerShell doesn't reformat the whole file
# and make the next upstream merge harder.
function Format-JsonNumber($v) {
    if ($v -is [double]) {
        if ($v -eq [math]::Floor($v)) { return "$([math]::Floor($v)).0" }
        return "$v"
    }
    return "$v"
}

function Format-JsonObjectText($obj, [int]$indent, [string]$nl) {
    $pad = ' ' * $indent
    $innerPad = ' ' * ($indent + 2)
    $lines = $obj.PSObject.Properties | ForEach-Object {
        $val = if ($_.Value -is [string]) { "`"$($_.Value)`"" } else { Format-JsonNumber $_.Value }
        "$innerPad`"$($_.Name)`": $val"
    }
    return "$pad{$nl" + ($lines -join ",$nl") + "$nl$pad}"
}

$configDefaultPath = Join-Path $root 'config-default.json'
$configDefaultText = Read-Utf8 $configDefaultPath
$configDefaultNl = Get-Newline $configDefaultText
foreach ($item in $items) {
    if (-not $item.config_default) { continue }
    foreach ($prop in $item.config_default.PSObject.Properties) {
        $key = $prop.Name
        $objText = Format-JsonObjectText $prop.Value 2 $configDefaultNl
        $entryPattern = '(?s)  "' + [regex]::Escape($key) + '":\s*\{.*?\r?\n  \}'
        $entryText = "  `"$key`": " + $objText.TrimStart()
        if ($configDefaultText -match $entryPattern) {
            $configDefaultText = [regex]::Replace($configDefaultText, $entryPattern, { param($m) $entryText }, 1)
        }
        else {
            # Add the new entry before the file's closing brace.
            $configDefaultText = $configDefaultText.TrimEnd()
            $configDefaultText = $configDefaultText.Substring(0, $configDefaultText.Length - 1).TrimEnd()
            if (-not $configDefaultText.EndsWith(',')) { $configDefaultText += ',' }
            $configDefaultText += "$configDefaultNl$entryText${configDefaultNl}}$configDefaultNl"
        }
    }
}
Write-Utf8NoBom $configDefaultPath $configDefaultText
Write-Host "[config-default.json] upserted sideloaded item defaults"

# Fill in missing items and fields in config.json
# Leave existing values alone; they may have been tuned by hand.
# Only use config_default for entries or fields that aren't there yet.
function Resolve-OptionTemplate([string]$text, $configObj) {
    # Keep the {field_name} syntax in sync with editor/src/template.rs.
    # Values come from the config object passed in by the caller.
    if (-not $text) { return $text }
    return [regex]::Replace($text, '\{(\w+)\}', {
            param($m)
            $name = $m.Groups[1].Value
            if ($configObj -and ($configObj.PSObject.Properties.Name -contains $name)) {
                # Show whole numbers as 250 rather than 250.0 in descriptions.
                # Handle both double and decimal values, keeping any fractional part.
                $v = $configObj.$name
                if (($v -is [double] -or $v -is [decimal]) -and $v -eq [math]::Floor($v)) {
                    return "$([math]::Floor($v))"
                }
                return "$v"
            }
            return "{?$name}"
        })
}

$configJsonPath = Join-Path $root 'config.json'
if (-not (Test-Path $configJsonPath)) {
    # No root config.json means the user hasn't opted into one yet.
    # Fall back to sideloader/config.json, bootstrapping it from
    # config-default.json if that doesn't exist either.
    $configJsonPath = Join-Path $sider 'config.json'
    if (-not (Test-Path $configJsonPath)) {
        Copy-Item (Join-Path $root 'config-default.json') $configJsonPath
        Write-Host "[config.json] no root config.json or sideloader/config.json found - bootstrapped sideloader/config.json from config-default.json"
    }
}
$configJsonText = Read-Utf8 $configJsonPath
$configJsonNl = Get-Newline $configJsonText
$configJsonParsed = $configJsonText | ConvertFrom-Json
$configJsonChangedKeys = 0
# Cache the merged values even if nothing changed. The item descriptions
# below need the live config values, including any manual balance tweaks.
$mergedConfigBySlug = @{}
foreach ($item in $items) {
    if (-not $item.config_default) { continue }
    foreach ($prop in $item.config_default.PSObject.Properties) {
        $key = $prop.Name
        $defaults = $prop.Value
        $existing = $configJsonParsed.PSObject.Properties[$key]
        $merged = [ordered]@{}
        if ($existing) {
            foreach ($p in $existing.Value.PSObject.Properties) { $merged[$p.Name] = $p.Value }
        }
        $addedAny = -not $existing
        foreach ($p in $defaults.PSObject.Properties) {
            if (-not $merged.Contains($p.Name)) {
                $merged[$p.Name] = $p.Value
                $addedAny = $true
            }
        }
        $mergedConfigBySlug[$key] = [PSCustomObject]$merged
        if (-not $addedAny) { continue }
        $configJsonChangedKeys++
        $objText = Format-JsonObjectText ([PSCustomObject]$merged) 2 $configJsonNl
        if ($existing) {
            # These entries are flat objects, so this handles both single-line and
            # multiline entries. The match leaves the leading spaces in place;
            # don't add another indent in the replacement.
            $entryPattern = '"' + [regex]::Escape($key) + '":\s*\{[^{}]*\}'
            $entryText = "`"$key`": " + $objText.TrimStart()
            $configJsonText = [regex]::Replace($configJsonText, $entryPattern, { param($m) $entryText }, 1)
        }
        else {
            $entryText = "  `"$key`": " + $objText.TrimStart()
            $configJsonText = $configJsonText.TrimEnd()
            $configJsonText = $configJsonText.Substring(0, $configJsonText.Length - 1).TrimEnd()
            if (-not $configJsonText.EndsWith(',')) { $configJsonText += ',' }
            $configJsonText += "$configJsonNl$entryText${configJsonNl}}$configJsonNl"
        }
    }
}
Write-Utf8NoBom $configJsonPath $configJsonText
Write-Host "[config.json] $configJsonChangedKeys item(s) had a new entry or missing key(s) added"

# Update names and descriptions in text/item.i18n
# Skip this when there's no text to add, since serializing the JSON rewrites
# the whole file. Keep the results in an array for the count check.
$itemsWithI18n = @($items | Where-Object { $_.i18n })
if ($itemsWithI18n.Count -gt 0) {
    $i18nPath = Join-Path $root 'text\item.i18n'
    $i18n = Read-Utf8 $i18nPath | ConvertFrom-Json
    foreach ($item in $itemsWithI18n) {
        # The manifest uses locale -> slug -> {name, option}.
        # Radiant variants have their own slugs, so copy those entries too.
        foreach ($localeProp in $item.i18n.PSObject.Properties) {
            $locale = $localeProp.Name
            if (-not ($i18n.PSObject.Properties.Name -contains $locale)) { continue }
            foreach ($slugProp in $localeProp.Value.PSObject.Properties) {
                $slugKey = $slugProp.Name
                $entry = $slugProp.Value
                # Fill the description's placeholders with this slug's merged config
                # values so the tooltip matches the game. Use manifest defaults
                # if there's no merged entry for this slug.
                $configForSlug = $mergedConfigBySlug[$slugKey]
                if (-not $configForSlug -and $item.config_default -and
                    ($item.config_default.PSObject.Properties.Name -contains $slugKey)) {
                    $configForSlug = $item.config_default.$slugKey
                }
                $resolvedEntry = [PSCustomObject]@{
                    name   = $entry.name
                    option = Resolve-OptionTemplate $entry.option $configForSlug
                }
                $i18n.$locale | Add-Member -NotePropertyName $slugKey -NotePropertyValue $resolvedEntry -Force
            }
        }
    }
    Write-Utf8NoBom $i18nPath (($i18n | ConvertTo-Json -Depth 10) -replace "`n", "`r`n")
    Write-Host "[item.i18n] upserted text for $($itemsWithI18n.Count) sideloaded item(s)"
}
else {
    Write-Host "[item.i18n] no sideloaded items have manifest text yet - left untouched"
}

# Add item icons to the shared sprite sheet
$sheetJsonPath = Join-Path $root 'aseprite_resources\ingame\item_icons_640X640#data.sprite_sheet'
$sheetPngPath = Join-Path $root 'aseprite_resources\ingame\item_icons_640X640#sheet.png'
$sheetJsonText = Read-Utf8 $sheetJsonPath
$sheetNl = Get-Newline $sheetJsonText
$sheetJson = $sheetJsonText | ConvertFrom-Json
$images = @{}
foreach ($p in $sheetJson.images.PSObject.Properties) { $images[$p.Name] = $p.Value }
$newSpriteEntries = @()

Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($sheetPngPath)
$sheetPx = $bmp.Width
# Get the cell size in pixels from an existing sprite's fractional width.
$sampleRect = $images.Values | Select-Object -First 1
$cellPx = [int]([double]$sampleRect.w * $sheetPx)
$cols = [int]($sheetPx / $cellPx)

function Find-FreeCell($images, $cols) {
    $occupied = @{}
    foreach ($v in $images.Values) {
        $col = [int]([math]::Round($v.x / (1.0 / $cols)))
        $row = [int]([math]::Round($v.y / (1.0 / $cols)))
        $occupied["$col,$row"] = $true
    }
    for ($row = 0; $row -lt $cols; $row++) {
        for ($col = 0; $col -lt $cols; $col++) {
            if (-not $occupied.ContainsKey("$col,$row")) { return @{ col = $col; row = $row } }
        }
    }
    throw "sprite sheet is full - no free cell for a new icon"
}

$dirty = $false
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$goldBorderPath = Join-Path $sider 'icons\radiant_border.png'

foreach ($item in $items) {
    if (-not $item.icon) { continue }
    $basePngPath = Join-Path $sider "icons\$($item.icon.base)"
    if (-not (Test-Path $basePngPath)) {
        Write-Warning "[icons] $($item.slug): no sideloader/icons/$($item.icon.base) provided - skipping icon injection"
        continue
    }
    if (-not $images.ContainsKey($item.slug)) {
        $cell = Find-FreeCell $images $cols
        $icon = [System.Drawing.Image]::FromFile($basePngPath)
        $graphics.DrawImage($icon, ($cell.col * $cellPx), ($cell.row * $cellPx), $cellPx, $cellPx)
        $icon.Dispose()
        $frac = 1.0 / $cols
        $rect = [ordered]@{ x = [math]::Round($cell.col * $frac, 4); y = [math]::Round($cell.row * $frac, 4); w = [math]::Round($frac, 4); h = [math]::Round($frac, 4) }
        $images[$item.slug] = $rect
        $newSpriteEntries += @{ slug = $item.slug; rect = $rect }
        $dirty = $true
        Write-Host "[icons] composited $($item.slug) into sheet at col=$($cell.col) row=$($cell.row)"
    }

    $radiantSlug = "radiant_$($item.slug)"
    if (-not $images.ContainsKey($radiantSlug)) {
        # No radiant filename means using the gold border below. Don't build
        # a path without it: the icons directory would pass Test-Path, then
        # FromFile would try to open the directory as an image.
        $radiantPngPath = if ($item.icon.radiant) { Join-Path $sider "icons\$($item.icon.radiant)" } else { $null }
        $cell = Find-FreeCell $images $cols
        if ($radiantPngPath -and (Test-Path $radiantPngPath)) {
            $icon = [System.Drawing.Image]::FromFile($radiantPngPath)
            $graphics.DrawImage($icon, ($cell.col * $cellPx), ($cell.row * $cellPx), $cellPx, $cellPx)
            $icon.Dispose()
        }
        elseif ((Test-Path $goldBorderPath)) {
            $icon = [System.Drawing.Image]::FromFile($basePngPath)
            $graphics.DrawImage($icon, ($cell.col * $cellPx), ($cell.row * $cellPx), $cellPx, $cellPx)
            $icon.Dispose()
            $border = [System.Drawing.Image]::FromFile($goldBorderPath)
            $graphics.DrawImage($border, ($cell.col * $cellPx), ($cell.row * $cellPx), $cellPx, $cellPx)
            $border.Dispose()
            Write-Host "[icons] synthesized radiant for $($item.slug) (base + radiant border)"
        }
        else {
            Write-Warning "[icons] $($item.slug): no radiant icon and no sideloader/icons/radiant_border.png to synthesize one - skipping"
            continue
        }
        $frac = 1.0 / $cols
        $rect = [ordered]@{ x = [math]::Round($cell.col * $frac, 4); y = [math]::Round($cell.row * $frac, 4); w = [math]::Round($frac, 4); h = [math]::Round($frac, 4) }
        $images[$radiantSlug] = $rect
        $newSpriteEntries += @{ slug = $radiantSlug; rect = $rect }
        $dirty = $true
    }
}
$graphics.Dispose()

if ($dirty) {
    $bmp.Save("$sheetPngPath.tmp", [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Move-Item "$sheetPngPath.tmp" $sheetPngPath -Force

    # Insert the new entries as text to preserve the rest of the formatting.
    $entryTexts = $newSpriteEntries | ForEach-Object {
        $r = $_.rect
        "        `"$($_.slug)`": {${sheetNl}            `"x`": $($r.x),${sheetNl}            `"y`": $($r.y),${sheetNl}            `"w`": $($r.w),${sheetNl}            `"h`": $($r.h)${sheetNl}        }"
    }
    $insertion = ($entryTexts -join ",$sheetNl")
    # Add a comma after the last entry, then insert the new entries
    # before the images object's closing brace.
    $imagesCloseRegex = '(?s)(\r?\n\s{8}\})(\r?\n\s{4}\}\r?\n\})\s*$'
    $sheetJsonText = [regex]::Replace($sheetJsonText, $imagesCloseRegex, { param($m) $m.Groups[1].Value + ",$sheetNl" + $insertion + $m.Groups[2].Value }, 1)
    Write-Utf8NoBom $sheetJsonPath $sheetJsonText
    Write-Host "[icons] sprite sheet updated"
}
else {
    $bmp.Dispose()
    Write-Host "[icons] no changes"
}

# Add /sideloader/ to .gitignore if needed
$gitignorePath = Join-Path $root '.gitignore'
$gitignore = Read-Utf8 $gitignorePath
$gitignoreNl = Get-Newline $gitignore
if ($gitignore -notmatch '(?m)^/sideloader/\s*$') {
    $gitignore = $gitignore.TrimEnd() + "$gitignoreNl$gitignoreNl# Siderloader: our custom/modified items, reapplied by sideloader/sideloader.ps1${gitignoreNl}/sideloader/$gitignoreNl"
    Write-Utf8NoBom $gitignorePath $gitignore
    Write-Host "[.gitignore] added /sideloader/"
}

Write-Host ""
Write-Host "sideloader.ps1 complete." -ForegroundColor Green
