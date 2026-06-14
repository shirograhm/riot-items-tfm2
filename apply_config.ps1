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
$ntFlat = if ($null -ne $config.nashors_tooth.effect_bonus_flat_damage) { [int]$config.nashors_tooth.effect_bonus_flat_damage }              else { 35 }
$ntApPct = if ($null -ne $config.nashors_tooth.effect_ap_percent_damage) { [double]$config.nashors_tooth.effect_ap_percent_damage }              else { 3.0 }
$rntFlat = if ($null -ne $config.radiant_nashors_tooth.effect_bonus_flat_damage) { [int]$config.radiant_nashors_tooth.effect_bonus_flat_damage } else { 50 }
$rntApPct = if ($null -ne $config.radiant_nashors_tooth.effect_ap_percent_damage) { [double]$config.radiant_nashors_tooth.effect_ap_percent_damage } else { 5.0 }
$rmHpPct = if ($null -ne $config.riftmaker.effect_caster_hp_percent_power) { [double]$config.riftmaker.effect_caster_hp_percent_power }              else { 1.0 }
$rmDur = if ($null -ne $config.riftmaker.effect_duration_seconds) { [int]$config.riftmaker.effect_duration_seconds }                             else { 5 }
$rmStacks = if ($null -ne $config.riftmaker.effect_max_stacks) { [int]$config.riftmaker.effect_max_stacks }                                      else { 3 }
$rrmHpPct = if ($null -ne $config.radiant_riftmaker.effect_caster_hp_percent_power) { [double]$config.radiant_riftmaker.effect_caster_hp_percent_power } else { 1.0 }
$rrmDur = if ($null -ne $config.radiant_riftmaker.effect_duration_seconds) { [int]$config.radiant_riftmaker.effect_duration_seconds }                 else { 5 }
$rrmStacks = if ($null -ne $config.radiant_riftmaker.effect_max_stacks) { [int]$config.radiant_riftmaker.effect_max_stacks }                          else { 3 }
$mrHeal = if ($null -ne $config.mortal_reminder.effect_heal_reduce) { [int]$config.mortal_reminder.effect_heal_reduce }              else { 40 }
$mrDur = if ($null -ne $config.mortal_reminder.effect_duration_seconds) { [int]$config.mortal_reminder.effect_duration_seconds }      else { 2 }
$rmrHeal = if ($null -ne $config.radiant_mortal_reminder.effect_heal_reduce) { [int]$config.radiant_mortal_reminder.effect_heal_reduce }    else { 40 }
$rmrDur = if ($null -ne $config.radiant_mortal_reminder.effect_duration_seconds) { [int]$config.radiant_mortal_reminder.effect_duration_seconds } else { 2 }
$jakDefMult = if ($null -ne $config.jaksho_the_protean.effect_stack_defence_mult) { [int]$config.jaksho_the_protean.effect_stack_defence_mult }                      else { 6 }
$jakMrMult = if ($null -ne $config.jaksho_the_protean.effect_stack_magic_resistance_mult) { [int]$config.jaksho_the_protean.effect_stack_magic_resistance_mult }        else { 6 }
$jakDur = if ($null -ne $config.jaksho_the_protean.effect_duration_seconds) { [int]$config.jaksho_the_protean.effect_duration_seconds }                                else { 4 }
$jakStacks = if ($null -ne $config.jaksho_the_protean.effect_max_stacks) { [int]$config.jaksho_the_protean.effect_max_stacks }                                         else { 4 }
$rjakDefMult = if ($null -ne $config.radiant_jaksho_the_protean.effect_stack_defence_mult) { [int]$config.radiant_jaksho_the_protean.effect_stack_defence_mult }        else { 10 }
$rjakMrMult = if ($null -ne $config.radiant_jaksho_the_protean.effect_stack_magic_resistance_mult) { [int]$config.radiant_jaksho_the_protean.effect_stack_magic_resistance_mult } else { 10 }
$rjakDur = if ($null -ne $config.radiant_jaksho_the_protean.effect_duration_seconds) { [int]$config.radiant_jaksho_the_protean.effect_duration_seconds }                else { 4 }
$rjakStacks = if ($null -ne $config.radiant_jaksho_the_protean.effect_max_stacks) { [int]$config.radiant_jaksho_the_protean.effect_max_stacks }                        else { 4 }
$fmSlow = if ($null -ne $config.frozen_mallet.effect_slow_amount) { [int]$config.frozen_mallet.effect_slow_amount }                              else { 25 }
$fmDur = if ($null -ne $config.frozen_mallet.effect_duration_seconds) { [int]$config.frozen_mallet.effect_duration_seconds }                          else { 2 }
$rfmSlow = if ($null -ne $config.radiant_frozen_mallet.effect_slow_amount) { [int]$config.radiant_frozen_mallet.effect_slow_amount }                else { 25 }
$rfmDur = if ($null -ne $config.radiant_frozen_mallet.effect_duration_seconds) { [int]$config.radiant_frozen_mallet.effect_duration_seconds }          else { 2 }
$rfmFlat = if ($null -ne $config.radiant_frozen_mallet.effect_bonus_flat_damage) { [int]$config.radiant_frozen_mallet.effect_bonus_flat_damage }       else { 20 }
$rfmHpPct = if ($null -ne $config.radiant_frozen_mallet.effect_caster_hp_percent_damage) { [double]$config.radiant_frozen_mallet.effect_caster_hp_percent_damage } else { 3.0 }
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
$borkPct = if ($null -ne $config.blade_of_the_ruined_king.effect_hp_percent_damage) { [double]$config.blade_of_the_ruined_king.effect_hp_percent_damage } else { 5.0 }
$borkCap = if ($null -ne $config.blade_of_the_ruined_king.effect_minion_damage_cap) { [int]$config.blade_of_the_ruined_king.effect_minion_damage_cap } else { 50 }
$rborkPct = if ($null -ne $config.radiant_blade_of_the_ruined_king.effect_hp_percent_damage) { [double]$config.radiant_blade_of_the_ruined_king.effect_hp_percent_damage } else { 5.0 }
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
$svHeal = if ($null -ne $config.spirit_visage.effect_heal_mult) { [double]$config.spirit_visage.effect_heal_mult }                             else { 20.0 }
$rsvHeal = if ($null -ne $config.radiant_spirit_visage.effect_heal_mult) { [double]$config.radiant_spirit_visage.effect_heal_mult }               else { 20.0 }
$udFlat = if ($null -ne $config.unending_despair.effect_bonus_flat_heal) { [int]$config.unending_despair.effect_bonus_flat_heal }              else { 10 }
$udHpPct = if ($null -ne $config.unending_despair.effect_caster_hp_percent_heal) { [double]$config.unending_despair.effect_caster_hp_percent_heal } else { 1.0 }
$rudFlat = if ($null -ne $config.radiant_unending_despair.effect_bonus_flat_heal) { [int]$config.radiant_unending_despair.effect_bonus_flat_heal } else { 25 }
$rudHpPct = if ($null -ne $config.radiant_unending_despair.effect_caster_hp_percent_heal) { [double]$config.radiant_unending_despair.effect_caster_hp_percent_heal } else { 3.0 }
$phFlat = if ($null -ne $config.protoplasm_harness.effect_bonus_flat_hp) { [int]$config.protoplasm_harness.effect_bonus_flat_hp }                      else { 300 }
$phHpPct = if ($null -ne $config.protoplasm_harness.effect_hp_percent_boost) { [double]$config.protoplasm_harness.effect_hp_percent_boost }                  else { 25.0 }
$phDur = if ($null -ne $config.protoplasm_harness.effect_duration_seconds) { [int]$config.protoplasm_harness.effect_duration_seconds }                    else { 6 }
$phCd = if ($null -ne $config.protoplasm_harness.effect_cooldown_seconds) { [int]$config.protoplasm_harness.effect_cooldown_seconds }                     else { 30 }
$rphFlat = if ($null -ne $config.radiant_protoplasm_harness.effect_bonus_flat_hp) { [int]$config.radiant_protoplasm_harness.effect_bonus_flat_hp }         else { 600 }
$rphHpPct = if ($null -ne $config.radiant_protoplasm_harness.effect_hp_percent_boost) { [double]$config.radiant_protoplasm_harness.effect_hp_percent_boost }  else { 25.0 }
$rphDur = if ($null -ne $config.radiant_protoplasm_harness.effect_duration_seconds) { [int]$config.radiant_protoplasm_harness.effect_duration_seconds }    else { 6 }
$rphCd = if ($null -ne $config.radiant_protoplasm_harness.effect_cooldown_seconds) { [int]$config.radiant_protoplasm_harness.effect_cooldown_seconds }     else { 30 }
$tPen = if ($null -ne $config.terminus.effect_pen_per_stack) { [int]$config.terminus.effect_pen_per_stack }                       else { 4 }
$tDur = if ($null -ne $config.terminus.effect_duration_seconds) { [int]$config.terminus.effect_duration_seconds }                 else { 4 }
$tStacks = if ($null -ne $config.terminus.effect_max_stacks) { [int]$config.terminus.effect_max_stacks }                          else { 4 }
$rtPen = if ($null -ne $config.radiant_terminus.effect_pen_per_stack) { [int]$config.radiant_terminus.effect_pen_per_stack }      else { 4 }
$rtDur = if ($null -ne $config.radiant_terminus.effect_duration_seconds) { [int]$config.radiant_terminus.effect_duration_seconds } else { 4 }
$rtStacks = if ($null -ne $config.radiant_terminus.effect_max_stacks) { [int]$config.radiant_terminus.effect_max_stacks }         else { 4 }

$i18n = Get-Content $i18nPath -Raw | ConvertFrom-Json

$adIcon = "i#asset/base/ui/banpick/champion_stat_icon:ad_0"
$armorIcon = "i#asset/base/ui/banpick/champion_stat_icon:armor_0"
$hpIcon = "i#asset/base/ui/banpick/champion_stat_icon:hp_0"
$mrIcon = "i#asset/base/ui/banpick/champion_stat_icon:magic resistance_0"
$apIcon = "i#asset/base/ui/banpick/champion_stat_icon:ap_0"
$asIcon = "i#asset/base/ui/banpick/champion_stat_icon:attack_speed_0"
$forceIcon = "i#asset/base/ui/banpick/champion_stat_icon:force_0"
$speedIcon = "i#asset/base/ui/banpick/champion_stat_icon:speed_0"
$cdrIcon = "i#asset/base/ui/banpick/champion_stat_icon:cdr_0"
$armorPenIcon = "i#asset/base/ui/banpick/champion_stat_icon:armor_pen_0"
$magicPenIcon = "i#asset/base/ui/banpick/champion_stat_icon:magic_pen_0"

$i18n.en.executioners_calling.option = "Executioner: On attack, <#d94c49ff>reduce healing by ${execHeal}%<> for <#e8a800ff>${execDur} seconds<>."
$i18n.en.nashors_tooth.option = "Icathian Bite: On attack, deal <#a974ffff>bonus magic damage<> equal to <#a974ffff>${ntFlat}<> + <#a974ffff>${ntApPct}%<> <$apIcon> <#a974ffff>Ability Power<>."
$i18n.en.radiant_nashors_tooth.option = "Icathian Bite: On attack, deal <#a974ffff>bonus magic damage<> equal to <#a974ffff>${rntFlat}<> + <#a974ffff>${rntApPct}%<> <$apIcon> <#a974ffff>Ability Power<>."
$i18n.en.riftmaker.option = "Infusion: Spell hits grant <$apIcon> <#a974ffff>Ability Power<> equal to <#60e84dff>${rmHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> for <#e8a800ff>${rmDur} seconds<> (max ${rmStacks} stacks)."
$i18n.en.radiant_riftmaker.option = "Infusion: Spell hits grant <$apIcon> <#a974ffff>Ability Power<> equal to <#60e84dff>${rrmHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> for <#e8a800ff>${rrmDur} seconds<> (max ${rrmStacks} stacks)."
$i18n.en.mortal_reminder.option = "Executioner: On attack, <#d94c49ff>reduce healing by ${mrHeal}%<> for <#e8a800ff>${mrDur} seconds<>."
$i18n.en.radiant_mortal_reminder.option = "Executioner: On attack, <#d94c49ff>reduce healing by ${rmrHeal}%<> for <#e8a800ff>${rmrDur} seconds<>."
$i18n.en.jaksho_the_protean.option = "Resilience: Taking damage from an enemy champion grants <#ffdd8eff>${jakDefMult}% <$armorIcon> armor<> and <#88ccffff>${jakMrMult}% <$mrIcon> magic resistance<> for <#e8a800ff>${jakDur} seconds<> (max ${jakStacks} stacks)."
$i18n.en.radiant_jaksho_the_protean.option = "Resilience: Taking damage from an enemy champion grants <#ffdd8eff>${rjakDefMult}% <$armorIcon> armor<> and <#88ccffff>${rjakMrMult}% <$mrIcon> magic resistance<> for <#e8a800ff>${rjakDur} seconds<> (max ${rjakStacks} stacks)."
$i18n.en.frozen_mallet.option = "Icy: On attack, apply a <#d94c49ff>${fmSlow}% slow<> for <#e8a800ff>${fmDur} seconds<>."
$i18n.en.radiant_frozen_mallet.option = "Icy: On attack, deal <#ff9028ff>bonus physical damage<> equal to <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> and apply a <#d94c49ff>${rfmSlow}% slow<> for <#e8a800ff>${rfmDur} seconds<>."
$i18n.en.experimental_hexplate.option = "Overdrive: Gain <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>cooldown reduction<> on your ultimate skill."
$i18n.en.radiant_experimental_hexplate.option = "Overdrive: Gain <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>cooldown reduction<> on your ultimate skill."
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
$i18n.en.spirit_visage.option = "Vitality: Increase all <#60e84dff>healing received<> by <#60e84dff>${svHeal}%<>."
$i18n.en.radiant_spirit_visage.option = "Vitality: Increase all <#60e84dff>healing received<> by <#60e84dff>${rsvHeal}%<>."
$i18n.en.unending_despair.option = "Anguish: Spell hits heal you for <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.radiant_unending_despair.option = "Anguish: Spell hits heal you for <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.protoplasm_harness.option = "Fortification: <#d94c49ff>Falling below 40% health<> grants <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> as <#60e84dff>bonus health<> for <#e8a800ff>${phDur} seconds<> and <#60e84dff>heals you<> for half that amount (${phCd} second cooldown)."
$i18n.en.radiant_protoplasm_harness.option = "Fortification: <#d94c49ff>Falling below 40% health<> grants <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> as <#60e84dff>bonus health<> for <#e8a800ff>${rphDur} seconds<> and <#60e84dff>heals you<> for half that amount (${rphCd} second cooldown)."
$i18n.en.terminus.option = "Juxtaposition: On attack, gain either <#ffdd8eff>${tPen}% <$armorPenIcon> armor penetration<> or <#88ccffff>${tPen}% <$magicPenIcon> magic resistance penetration<> for <#e8a800ff>${tDur} seconds<>, alternating (max ${tStacks} stacks each)."
$i18n.en.radiant_terminus.option = "Juxtaposition: On attack, gain either <#ffdd8eff>${rtPen}% <$armorPenIcon> armor penetration<> or <#88ccffff>${rtPen}% <$magicPenIcon> magic resistance penetration<> for <#e8a800ff>${rtDur} seconds<>, alternating (max ${rtStacks} stacks each)."

[System.IO.File]::WriteAllText($i18nPath, ($i18n | ConvertTo-Json -Depth 10))

Write-Host "Done."
Write-Host "  Executioner's Calling:   -${execHeal}% healing / ${execDur}s"
Write-Host "  Nashor's Tooth:         ${ntFlat} + ${ntApPct}% AP magic dmg"
Write-Host "  Radiant Nashor's Tooth: ${rntFlat} + ${rntApPct}% AP magic dmg"
Write-Host "  Riftmaker:         ${rmHpPct}% max HP AP/stack / ${rmDur}s / ${rmStacks} stacks"
Write-Host "  Radiant Riftmaker: ${rrmHpPct}% max HP AP/stack / ${rrmDur}s / ${rrmStacks} stacks"
Write-Host "  Mortal Reminder:         -${mrHeal}% healing / ${mrDur}s"
Write-Host "  Radiant Mortal Reminder: -${rmrHeal}% healing / ${rmrDur}s"
Write-Host "  Jak'sho:         ${jakDefMult}% armor+MR/stack / ${jakDur}s / ${jakStacks} stacks"
Write-Host "  Radiant Jak'sho: ${rjakDefMult}% armor+MR/stack / ${rjakDur}s / ${rjakStacks} stacks"
Write-Host "  Frozen Mallet:         ${fmSlow}% slow / ${fmDur}s"
Write-Host "  Radiant Frozen Mallet: ${rfmFlat} + ${rfmHpPct}% max HP dmg / ${rfmSlow}% slow / ${rfmDur}s"
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
Write-Host "  Spirit Visage:              +${svHeal}% healing"
Write-Host "  Radiant Spirit Visage:      +${rsvHeal}% healing"
Write-Host "  Unending Despair:           ${udFlat} + ${udHpPct}% max HP heal/skill"
Write-Host "  Radiant Unending Despair:   ${rudFlat} + ${rudHpPct}% max HP heal/skill"
Write-Host "  Protoplasm Harness:         ${phFlat} + ${phHpPct}% max HP / ${phDur}s / ${phCd}s cooldown"
Write-Host "  Radiant Protoplasm Harness: ${rphFlat} + ${rphHpPct}% max HP / ${rphDur}s / ${rphCd}s cooldown"
Write-Host "  Terminus:               ${tPen}% pen/stack / ${tDur}s / ${tStacks} stacks"
Write-Host "  Radiant Terminus:       ${rtPen}% pen/stack / ${rtDur}s / ${rtStacks} stacks"
