$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$configPath = Join-Path $scriptDir "config.json"
$i18nPath = Join-Path $scriptDir "text\item.i18n"

if (-not (Test-Path $configPath)) {
    Write-Host "config.json not found, using defaults."
    $config = [PSCustomObject]@{}
}
else {
    $config = Get-Content $configPath -Raw | ConvertFrom-Json
}

$execHeal = if ($null -ne $config.executioners_calling.effect_heal_reduce) { [int]$config.executioners_calling.effect_heal_reduce }           else { 25 }
$execDur = if ($null -ne $config.executioners_calling.effect_duration_seconds) { [int]$config.executioners_calling.effect_duration_seconds } else { 2 }
$hexUltCdr = if ($null -ne $config.experimental_hexplate.ult_cooldown_mult) { [int]$config.experimental_hexplate.ult_cooldown_mult }              else { 15 }
$rhexUltCdr = if ($null -ne $config.radiant_experimental_hexplate.ult_cooldown_mult) { [int]$config.radiant_experimental_hexplate.ult_cooldown_mult } else { 25 }
$gbDmg = if ($null -ne $config.guinsoos_rageblade.effect_bonus_magic_damage) { [int]$config.guinsoos_rageblade.effect_bonus_magic_damage }              else { 30 }
$gbSpeed = if ($null -ne $config.guinsoos_rageblade.effect_stack_attack_speed_mult) { [int]$config.guinsoos_rageblade.effect_stack_attack_speed_mult }  else { 8 }
$gbDur = if ($null -ne $config.guinsoos_rageblade.effect_duration_seconds) { [int]$config.guinsoos_rageblade.effect_duration_seconds }                 else { 4 }
$gbStacks = if ($null -ne $config.guinsoos_rageblade.effect_max_stacks) { [int]$config.guinsoos_rageblade.effect_max_stacks }                          else { 4 }
$rgbDmg = if ($null -ne $config.radiant_guinsoos_rageblade.effect_bonus_magic_damage) { [int]$config.radiant_guinsoos_rageblade.effect_bonus_magic_damage }              else { 30 }
$rgbSpeed = if ($null -ne $config.radiant_guinsoos_rageblade.effect_stack_attack_speed_mult) { [int]$config.radiant_guinsoos_rageblade.effect_stack_attack_speed_mult }  else { 8 }
$rgbDur = if ($null -ne $config.radiant_guinsoos_rageblade.effect_duration_seconds) { [int]$config.radiant_guinsoos_rageblade.effect_duration_seconds }                 else { 4 }
$rgbStacks = if ($null -ne $config.radiant_guinsoos_rageblade.effect_max_stacks) { [int]$config.radiant_guinsoos_rageblade.effect_max_stacks }                          else { 4 }
$bftPower = if ($null -ne $config.blackfire_torch.effect_stack_magic_power) { [int]$config.blackfire_torch.effect_stack_magic_power }              else { 10 }
$bftDur = if ($null -ne $config.blackfire_torch.effect_duration_seconds) { [int]$config.blackfire_torch.effect_duration_seconds }                  else { 4 }
$bftStacks = if ($null -ne $config.blackfire_torch.effect_max_stacks) { [int]$config.blackfire_torch.effect_max_stacks }                           else { 4 }
$rbftPower = if ($null -ne $config.radiant_blackfire_torch.effect_stack_magic_power) { [int]$config.radiant_blackfire_torch.effect_stack_magic_power } else { 30 }
$rbftDur = if ($null -ne $config.radiant_blackfire_torch.effect_duration_seconds) { [int]$config.radiant_blackfire_torch.effect_duration_seconds }    else { 4 }
$rbftStacks = if ($null -ne $config.radiant_blackfire_torch.effect_max_stacks) { [int]$config.radiant_blackfire_torch.effect_max_stacks }             else { 4 }
$borkPct = if ($null -ne $config.blade_of_the_ruined_king.effect_hp_percent_damage) { [int]$config.blade_of_the_ruined_king.effect_hp_percent_damage } else { 5 }
$borkCap = if ($null -ne $config.blade_of_the_ruined_king.effect_minion_damage_cap) { [int]$config.blade_of_the_ruined_king.effect_minion_damage_cap } else { 50 }
$rborkPct = if ($null -ne $config.radiant_blade_of_the_ruined_king.effect_hp_percent_damage) { [int]$config.radiant_blade_of_the_ruined_king.effect_hp_percent_damage } else { 5 }
$rborkCap = if ($null -ne $config.radiant_blade_of_the_ruined_king.effect_minion_damage_cap) { [int]$config.radiant_blade_of_the_ruined_king.effect_minion_damage_cap } else { 50 }
$dbMult = if ($null -ne $config.deathblade.attack_mult) { [int]$config.deathblade.attack_mult }                      else { 15 }
$rdbMult = if ($null -ne $config.radiant_deathblade.attack_mult) { [int]$config.radiant_deathblade.attack_mult }              else { 25 }
$rabMult = if ($null -ne $config.rabadons_deathcap.magic_power_mult) { [int]$config.rabadons_deathcap.magic_power_mult }          else { 20 }
$radRabMult = if ($null -ne $config.radiant_rabadons_deathcap.magic_power_mult) { [int]$config.radiant_rabadons_deathcap.magic_power_mult }  else { 35 }
$mbForce = if ($null -ne $config.mirage_blade.adaptive_force) { [int]$config.mirage_blade.adaptive_force }                else { 60 }
$rmbForce = if ($null -ne $config.radiant_mirage_blade.adaptive_force) { [int]$config.radiant_mirage_blade.adaptive_force }        else { 100 }
$mbMoveSpeed = if ($null -ne $config.mirage_blade.effect_move_speed_mult) { [int]$config.mirage_blade.effect_move_speed_mult }           else { 20 }
$rmbMoveSpeed = if ($null -ne $config.radiant_mirage_blade.effect_move_speed_mult) { [int]$config.radiant_mirage_blade.effect_move_speed_mult }  else { 20 }
$mbDuration = if ($null -ne $config.mirage_blade.effect_duration_seconds) { [int]$config.mirage_blade.effect_duration_seconds }          else { 2 }
$rmbDuration = if ($null -ne $config.radiant_mirage_blade.effect_duration_seconds) { [int]$config.radiant_mirage_blade.effect_duration_seconds }  else { 2 }

$i18n = Get-Content $i18nPath -Raw | ConvertFrom-Json

$adIcon = "i#asset/base/ui/banpick/champion_stat_icon:ad_0"
$apIcon = "i#asset/base/ui/banpick/champion_stat_icon:ap_0"
$asIcon = "i#asset/base/ui/banpick/champion_stat_icon:attack_speed_0"
$forceIcon = "i#asset/base/ui/banpick/champion_stat_icon:force_0"
$speedIcon = "i#asset/base/ui/banpick/champion_stat_icon:speed_0"

$i18n.en.executioners_calling.option = "Executioner: On attack, <#d94c49ff>reduce healing by ${execHeal}%<> for <#e8a800ff>${execDur} seconds<>."
$i18n.en.experimental_hexplate.option = "Overdrive: Reduce the cooldown of your ultimate skill by <#e8a800ff>${hexUltCdr}%<>."
$i18n.en.radiant_experimental_hexplate.option = "Overdrive: Reduce the cooldown of your ultimate skill by <#e8a800ff>${rhexUltCdr}%<>."
$i18n.en.guinsoos_rageblade.option = "Wrath: Your basic attacks deal <#a974ffff>${gbDmg} bonus magic damage<>.`n`nSeething Strike: On attack, gain <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>attack speed<> for <#e8a800ff>${gbDur} seconds<> (max ${gbStacks} stacks)."
$i18n.en.radiant_guinsoos_rageblade.option = "Wrath: Your basic attacks deal <#a974ffff>${rgbDmg} bonus magic damage<>.`n`nSeething Strike: On attack, gain <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>attack speed<> for <#e8a800ff>${rgbDur} seconds<> (max ${rgbStacks} stacks)."
$i18n.en.blackfire_torch.option = "Maleficent: Spell hits grant <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>Ability Power<> for <#e8a800ff>${bftDur} seconds<> (max ${bftStacks} stacks)."
$i18n.en.radiant_blackfire_torch.option = "Maleficent: Spell hits grant <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>Ability Power<> for <#e8a800ff>${rbftDur} seconds<> (max ${rbftStacks} stacks)."
$i18n.en.blade_of_the_ruined_king.option = "Mist's Edge: On attack, deal <#ff9028ff>bonus physical damage<> equal to <#d94c49ff>${borkPct}% of the target's current health<>. Deals a maximum of <#ff9028ff>${borkCap} physical damage<> against minions and monsters."
$i18n.en.radiant_blade_of_the_ruined_king.option = "Mist's Edge: On attack, deal <#ff9028ff>bonus physical damage<> equal to <#d94c49ff>${rborkPct}% of the target's current health<>. Deals a maximum of <#ff9028ff>${rborkCap} physical damage<> against minions and monsters."
$i18n.en.deathblade.option = "Apex: Increase your total <$adIcon> <#ff9028ff>Attack Damage<> by <#ff9028ff>${dbMult}%<>."
$i18n.en.radiant_deathblade.option = "Apex: Increase your total <$adIcon> <#ff9028ff>Attack Damage<> by <#ff9028ff>${rdbMult}%<>."
$i18n.en.rabadons_deathcap.option = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${rabMult}%<>."
$i18n.en.radiant_rabadons_deathcap.option = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${radRabMult}%<>."

$mbIllusion = "Illusion: Gain <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Adaptive Force<>. Each <$forceIcon> <#d48294ff>Adaptive Force<> grants <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Attack Damage<> or <#a974ffff>1<> <$apIcon> <#a974ffff>Ability Power<>, depending on which is higher.`n`nBlur: On kill, gain <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>{2} seconds<>."
$i18n.en.mirage_blade.option = $mbIllusion -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.en.radiant_mirage_blade.option = $mbIllusion -f $rmbForce, $rmbMoveSpeed, $rmbDuration

[System.IO.File]::WriteAllText($i18nPath, ($i18n | ConvertTo-Json -Depth 10))

Write-Host "Done."
Write-Host "  Executioner's Calling:   -${execHeal}% healing / ${execDur}s"
Write-Host "  Experimental Hexplate:         ${hexUltCdr}% ult CDR"
Write-Host "  Radiant Experimental Hexplate: ${rhexUltCdr}% ult CDR"
Write-Host "  Guinsoo's Rageblade:         ${gbDmg} magic dmg / ${gbSpeed}% AS/stack / ${gbDur}s / ${gbStacks} stacks"
Write-Host "  Radiant Guinsoo's Rageblade: ${rgbDmg} magic dmg / ${rgbSpeed}% AS/stack / ${rgbDur}s / ${rgbStacks} stacks"
Write-Host "  Blackfire Torch:         ${bftPower} AP/stack / ${bftDur}s / ${bftStacks} stacks"
Write-Host "  Radiant Blackfire Torch: ${rbftPower} AP/stack / ${rbftDur}s / ${rbftStacks} stacks"
Write-Host "  Blade of the Ruined King:         ${borkPct}% HP / ${borkCap} cap"
Write-Host "  Radiant Blade of the Ruined King: ${rborkPct}% HP / ${rborkCap} cap"
Write-Host "  Deathblade:              ${dbMult}%"
Write-Host "  Radiant Deathblade:      ${rdbMult}%"
Write-Host "  Rabadon's Deathcap:      ${rabMult}%"
Write-Host "  Radiant Rabadon's:       ${radRabMult}%"
Write-Host "  Mirage Blade:            ${mbForce} force"
Write-Host "  Radiant Mirage Blade:    ${rmbForce} force"
