$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$configPath = Join-Path $scriptDir "config.json"
$i18nPath   = Join-Path $scriptDir "text\item.i18n"

if (-not (Test-Path $configPath)) {
    Write-Host "config.json not found, using defaults."
    $config = [PSCustomObject]@{}
} else {
    $config = Get-Content $configPath -Raw | ConvertFrom-Json
}

$dbMult     = if ($null -ne $config.deathblade.attack_mult)                      { [int]$config.deathblade.attack_mult }                      else { 15 }
$rdbMult    = if ($null -ne $config.radiant_deathblade.attack_mult)              { [int]$config.radiant_deathblade.attack_mult }              else { 25 }
$rabMult    = if ($null -ne $config.rabadons_deathcap.magic_power_mult)          { [int]$config.rabadons_deathcap.magic_power_mult }          else { 20 }
$radRabMult = if ($null -ne $config.radiant_rabadons_deathcap.magic_power_mult)  { [int]$config.radiant_rabadons_deathcap.magic_power_mult }  else { 35 }

$i18n = Get-Content $i18nPath -Raw | ConvertFrom-Json

$adIcon = "i#asset/base/ui/banpick/champion_stat_icon:ad_0"
$apIcon = "i#asset/base/ui/banpick/champion_stat_icon:ap_0"

$i18n.en.deathblade.option                 = "Apex: Increase your total <$adIcon> <#ff9028ff>Attack Damage<> by <#ff9028ff>${dbMult}%<>."
$i18n.en.radiant_deathblade.option         = "Apex: Increase your total <$adIcon> <#ff9028ff>Attack Damage<> by <#ff9028ff>${rdbMult}%<>."
$i18n.en.rabadons_deathcap.option          = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${rabMult}%<>."
$i18n.en.radiant_rabadons_deathcap.option  = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${radRabMult}%<>."

[System.IO.File]::WriteAllText($i18nPath, ($i18n | ConvertTo-Json -Depth 10))

Write-Host "Done."
Write-Host "  Deathblade:              ${dbMult}%"
Write-Host "  Radiant Deathblade:      ${rdbMult}%"
Write-Host "  Rabadon's Deathcap:      ${rabMult}%"
Write-Host "  Radiant Rabadon's:       ${radRabMult}%"
