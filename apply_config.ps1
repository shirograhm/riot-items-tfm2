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
$ooHeal = if ($null -ne $config.oblivion_orb.effect_heal_reduce) { [int]$config.oblivion_orb.effect_heal_reduce }                             else { 25 }
$ooDur = if ($null -ne $config.oblivion_orb.effect_duration_seconds) { [int]$config.oblivion_orb.effect_duration_seconds }                    else { 2 }
$obmAtk = if ($null -ne $config.overlords_bloodmail.effect_caster_hp_percent_attack) { [double]$config.overlords_bloodmail.effect_caster_hp_percent_attack }                else { 1.5 }
$robmAtk = if ($null -ne $config.radiant_overlords_bloodmail.effect_caster_hp_percent_attack) { [double]$config.radiant_overlords_bloodmail.effect_caster_hp_percent_attack } else { 1.5 }
$nhFlat = if ($null -ne $config.night_harvester.effect_bonus_flat_damage) { [int]$config.night_harvester.effect_bonus_flat_damage }                      else { 160 }
$nhApPct = if ($null -ne $config.night_harvester.effect_ap_percent_damage) { [double]$config.night_harvester.effect_ap_percent_damage }                 else { 40.0 }
$nhMs = if ($null -ne $config.night_harvester.effect_move_speed_mult) { [int]$config.night_harvester.effect_move_speed_mult }                            else { 40 }
$nhDur = if ($null -ne $config.night_harvester.effect_duration_seconds) { [int]$config.night_harvester.effect_duration_seconds }                         else { 2 }
$nhCd = if ($null -ne $config.night_harvester.effect_cooldown_seconds) { [int]$config.night_harvester.effect_cooldown_seconds }                          else { 10 }
$rnhFlat = if ($null -ne $config.radiant_night_harvester.effect_bonus_flat_damage) { [int]$config.radiant_night_harvester.effect_bonus_flat_damage }    else { 160 }
$rnhApPct = if ($null -ne $config.radiant_night_harvester.effect_ap_percent_damage) { [double]$config.radiant_night_harvester.effect_ap_percent_damage } else { 40.0 }
$rnhMs = if ($null -ne $config.radiant_night_harvester.effect_move_speed_mult) { [int]$config.radiant_night_harvester.effect_move_speed_mult }           else { 40 }
$rnhDur = if ($null -ne $config.radiant_night_harvester.effect_duration_seconds) { [int]$config.radiant_night_harvester.effect_duration_seconds }       else { 2 }
$rnhCd = if ($null -ne $config.radiant_night_harvester.effect_cooldown_seconds) { [int]$config.radiant_night_harvester.effect_cooldown_seconds }        else { 10 }
$pvFlat = if ($null -ne $config.protectors_vow.effect_bonus_flat_hp) { [int]$config.protectors_vow.effect_bonus_flat_hp }                                else { 50 }
$pvArmorPct = if ($null -ne $config.protectors_vow.effect_caster_defence_percent_hp) { [double]$config.protectors_vow.effect_caster_defence_percent_hp }             else { 80.0 }
$rpvFlat = if ($null -ne $config.radiant_protectors_vow.effect_bonus_flat_hp) { [int]$config.radiant_protectors_vow.effect_bonus_flat_hp }               else { 50 }
$rpvArmorPct = if ($null -ne $config.radiant_protectors_vow.effect_caster_defence_percent_hp) { [double]$config.radiant_protectors_vow.effect_caster_defence_percent_hp } else { 80.0 }
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
$morHeal = if ($null -ne $config.morellonomicon.effect_heal_reduce) { [int]$config.morellonomicon.effect_heal_reduce }                              else { 40 }
$morDur = if ($null -ne $config.morellonomicon.effect_duration_seconds) { [int]$config.morellonomicon.effect_duration_seconds }                    else { 2 }
$rmorHeal = if ($null -ne $config.radiant_morellonomicon.effect_heal_reduce) { [int]$config.radiant_morellonomicon.effect_heal_reduce }            else { 40 }
$rmorDur = if ($null -ne $config.radiant_morellonomicon.effect_duration_seconds) { [int]$config.radiant_morellonomicon.effect_duration_seconds }  else { 2 }
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
$ddDelay = if ($null -ne $config.deaths_dance.effect_delayed_damage_percent) { [double]$config.deaths_dance.effect_delayed_damage_percent }              else { 25.0 }
$ddBurnCap = if ($null -ne $config.deaths_dance.effect_burn_hp_percent_cap) { [double]$config.deaths_dance.effect_burn_hp_percent_cap }                    else { 5.0 }
$rddDelay = if ($null -ne $config.radiant_deaths_dance.effect_delayed_damage_percent) { [double]$config.radiant_deaths_dance.effect_delayed_damage_percent } else { 25.0 }
$rddBurnCap = if ($null -ne $config.radiant_deaths_dance.effect_burn_hp_percent_cap) { [double]$config.radiant_deaths_dance.effect_burn_hp_percent_cap }      else { 5.0 }
$ddHeal = if ($null -ne $config.deaths_dance.effect_kill_heal_missing_percent) { [double]$config.deaths_dance.effect_kill_heal_missing_percent }              else { 15.0 }
$rddHeal = if ($null -ne $config.radiant_deaths_dance.effect_kill_heal_missing_percent) { [double]$config.radiant_deaths_dance.effect_kill_heal_missing_percent } else { 15.0 }
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
$phThreshold = if ($null -ne $config.protoplasm_harness.effect_hp_percent_threshold) { [int]$config.protoplasm_harness.effect_hp_percent_threshold }                      else { 40.0 }
$phFlat = if ($null -ne $config.protoplasm_harness.effect_bonus_flat_hp) { [int]$config.protoplasm_harness.effect_bonus_flat_hp }                      else { 300 }
$phHpPct = if ($null -ne $config.protoplasm_harness.effect_hp_percent_boost) { [double]$config.protoplasm_harness.effect_hp_percent_boost }                  else { 25.0 }
$phDur = if ($null -ne $config.protoplasm_harness.effect_duration_seconds) { [int]$config.protoplasm_harness.effect_duration_seconds }                    else { 6 }
$phCd = if ($null -ne $config.protoplasm_harness.effect_cooldown_seconds) { [int]$config.protoplasm_harness.effect_cooldown_seconds }                     else { 30 }
$rphThreshold = if ($null -ne $config.radiant_protoplasm_harness.effect_hp_percent_threshold) { [int]$config.radiant_protoplasm_harness.effect_hp_percent_threshold }                      else { 40.0 }
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
$colThreshold = if ($null -ne $config.collector.effect_hp_percent_threshold) { [double]$config.collector.effect_hp_percent_threshold }                  else { 8.0 }
$rcolThreshold = if ($null -ne $config.radiant_collector.effect_hp_percent_threshold) { [double]$config.radiant_collector.effect_hp_percent_threshold } else { 8.0 }
$hsFlat = if ($null -ne $config.heartsteel.effect_bonus_flat_damage) { [int]$config.heartsteel.effect_bonus_flat_damage }                              else { 15 }
$hsHpPct = if ($null -ne $config.heartsteel.effect_caster_hp_percent_damage) { [double]$config.heartsteel.effect_caster_hp_percent_damage }            else { 6.0 }
$hsBonusHpPct = if ($null -ne $config.heartsteel.effect_bonus_hp_percent_of_damage) { [double]$config.heartsteel.effect_bonus_hp_percent_of_damage }   else { 15.0 }
$hsCd = if ($null -ne $config.heartsteel.effect_cooldown_seconds) { [int]$config.heartsteel.effect_cooldown_seconds }                                  else { 15 }
$rhsFlat = if ($null -ne $config.radiant_heartsteel.effect_bonus_flat_damage) { [int]$config.radiant_heartsteel.effect_bonus_flat_damage }              else { 15 }
$rhsHpPct = if ($null -ne $config.radiant_heartsteel.effect_caster_hp_percent_damage) { [double]$config.radiant_heartsteel.effect_caster_hp_percent_damage }            else { 6.0 }
$rhsBonusHpPct = if ($null -ne $config.radiant_heartsteel.effect_bonus_hp_percent_of_damage) { [double]$config.radiant_heartsteel.effect_bonus_hp_percent_of_damage }   else { 15.0 }
$rhsCd = if ($null -ne $config.radiant_heartsteel.effect_cooldown_seconds) { [int]$config.radiant_heartsteel.effect_cooldown_seconds }                                  else { 15 }
$sosAtkMult = if ($null -ne $config.spear_of_shojin.effect_stack_attack_mult) { [int]$config.spear_of_shojin.effect_stack_attack_mult }                  else { 3 }
$sosDur = if ($null -ne $config.spear_of_shojin.effect_duration_seconds) { [int]$config.spear_of_shojin.effect_duration_seconds }                        else { 5 }
$sosStacks = if ($null -ne $config.spear_of_shojin.effect_max_stacks) { [int]$config.spear_of_shojin.effect_max_stacks }                                 else { 4 }
$rsosAtkMult = if ($null -ne $config.radiant_spear_of_shojin.effect_stack_attack_mult) { [int]$config.radiant_spear_of_shojin.effect_stack_attack_mult } else { 3 }
$rsosDur = if ($null -ne $config.radiant_spear_of_shojin.effect_duration_seconds) { [int]$config.radiant_spear_of_shojin.effect_duration_seconds }       else { 5 }
$rsosStacks = if ($null -ne $config.radiant_spear_of_shojin.effect_max_stacks) { [int]$config.radiant_spear_of_shojin.effect_max_stacks }                else { 4 }

$i18n = Get-Content $i18nPath -Raw -Encoding UTF8 | ConvertFrom-Json

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

Write-Host "Updating English text."

$i18n.en.executioners_calling.option = "Executioner: On attack, <#d94c49ff>reduce healing by ${execHeal}%<> for <#e8a800ff>${execDur} seconds<>."
$i18n.en.oblivion_orb.option = "Grievous Wounds: Dealing <#a974ffff>magic damage<> to an enemy champion <#d94c49ff>reduces their healing by ${ooHeal}%<> for <#e8a800ff>${ooDur} seconds<>."
$i18n.en.morellonomicon.option = "Grievous Wounds: Dealing <#a974ffff>magic damage<> to an enemy champion <#d94c49ff>reduces their healing by ${morHeal}%<> for <#e8a800ff>${morDur} seconds<>."
$i18n.en.radiant_morellonomicon.option = "Grievous Wounds: Dealing <#a974ffff>magic damage<> to an enemy champion <#d94c49ff>reduces their healing by ${rmorHeal}%<> for <#e8a800ff>${rmorDur} seconds<>."
$i18n.en.overlords_bloodmail.option = "Tyranny: Gain <#ff9028ff>bonus<> <$adIcon> <#ff9028ff>Attack Damage<> equal to <#60e84dff>${obmAtk}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.radiant_overlords_bloodmail.option = "Tyranny: Gain <#ff9028ff>bonus<> <$adIcon> <#ff9028ff>Attack Damage<> equal to <#60e84dff>${robmAtk}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.night_harvester.option = "Soulrend: Damaging an enemy champion deals <#a974ffff>${nhFlat}<> + <#a974ffff>${nhApPct}%<> <$apIcon> <#a974ffff>Ability Power<> as <#a974ffff>bonus magic damage<> and grants <#ffffffff>${nhMs}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>${nhDur} seconds<> (${nhCd} second cooldown per target)."
$i18n.en.radiant_night_harvester.option = "Soulrend: Damaging an enemy champion deals <#a974ffff>${rnhFlat}<> + <#a974ffff>${rnhApPct}%<> <$apIcon> <#a974ffff>Ability Power<> as <#a974ffff>bonus magic damage<> and grants <#ffffffff>${rnhMs}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>${rnhDur} seconds<> (${rnhCd} second cooldown per target)."
$i18n.en.protectors_vow.option = "Awe: Gain <#60e84dff>bonus health<> equal to <#60e84dff>${pvFlat}<> + <#ffdd8eff>${pvArmorPct}%<> of your <$armorIcon> <#ffdd8eff>Armor<>."
$i18n.en.radiant_protectors_vow.option = "Awe: Gain <#60e84dff>bonus health<> equal to <#60e84dff>${rpvFlat}<> + <#ffdd8eff>${rpvArmorPct}%<> of your <$armorIcon> <#ffdd8eff>Armor<>."
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
$i18n.en.deaths_dance.option = "Ignore Pain: <#e8a800ff>${ddDelay}%<> of damage taken is stored and dealt back to you over time as <#d94c49ff>bonus physical damage<> (up to <#60e84dff>${ddBurnCap}%<> of your <$hpIcon> <#60e84dff>maximum health<> per second).`n`nDefy: On kill, the stored damage is cleared and you <#60e84dff>heal<> for <#60e84dff>${ddHeal}%<> of your <#60e84dff>missing health<>."
$i18n.en.radiant_deaths_dance.option = "Ignore Pain: <#e8a800ff>${rddDelay}%<> of damage taken is stored and dealt back to you over time as <#d94c49ff>bonus physical damage<> (up to <#60e84dff>${rddBurnCap}%<> of your <$hpIcon> <#60e84dff>maximum health<> per second).`n`nDefy: On kill, the stored damage is cleared and you <#60e84dff>heal<> for <#60e84dff>${rddHeal}%<> of your <#60e84dff>missing health<>."
$i18n.en.rabadons_deathcap.option = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${rabMult}%<>."
$i18n.en.radiant_rabadons_deathcap.option = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${radRabMult}%<>."

$mbIllusion = "Illusion: Gain <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Adaptive Force<>. Each <$forceIcon> <#d48294ff>Adaptive Force<> grants <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Attack Damage<> or <#a974ffff>1<> <$apIcon> <#a974ffff>Ability Power<>, depending on which is higher.`n`nBlur: On kill, gain <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>{2} seconds<>."
$i18n.en.mirage_blade.option = $mbIllusion -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.en.radiant_mirage_blade.option = $mbIllusion -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$i18n.en.spirit_visage.option = "Vitality: Increase all <#60e84dff>healing received<> by <#60e84dff>${svHeal}%<>."
$i18n.en.radiant_spirit_visage.option = "Vitality: Increase all <#60e84dff>healing received<> by <#60e84dff>${rsvHeal}%<>."
$i18n.en.unending_despair.option = "Anguish: Spell hits heal you for <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.radiant_unending_despair.option = "Anguish: Spell hits heal you for <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.protoplasm_harness.option = "Fortification: <#d94c49ff>Falling below ${phThreshold}% health<> grants <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> as <#60e84dff>bonus health<> for <#e8a800ff>${phDur} seconds<> and <#60e84dff>heals you<> for half that amount (${phCd} second cooldown)."
$i18n.en.radiant_protoplasm_harness.option = "Fortification: <#d94c49ff>Falling below ${rphThreshold}% health<> grants <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> as <#60e84dff>bonus health<> for <#e8a800ff>${rphDur} seconds<> and <#60e84dff>heals you<> for half that amount (${rphCd} second cooldown)."
$i18n.en.terminus.option = "Juxtaposition: On attack, gain either <#ffdd8eff>${tPen}% <$armorPenIcon> armor penetration<> or <#88ccffff>${tPen}% <$magicPenIcon> magic resistance penetration<> for <#e8a800ff>${tDur} seconds<>, alternating (max ${tStacks} stacks each)."
$i18n.en.radiant_terminus.option = "Juxtaposition: On attack, gain either <#ffdd8eff>${rtPen}% <$armorPenIcon> armor penetration<> or <#88ccffff>${rtPen}% <$magicPenIcon> magic resistance penetration<> for <#e8a800ff>${rtDur} seconds<>, alternating (max ${rtStacks} stacks each)."
$i18n.en.collector.option = "Death: Dealing damage to enemy champions below <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>maximum health<> <#d94c49ff>executes<> them."
$i18n.en.radiant_collector.option = "Death: Dealing damage to enemy champions below <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#60e84dff>maximum health<> <#d94c49ff>executes<> them."
$i18n.en.heartsteel.option = "Ironheart: Every <#e8a800ff>${hsCd} seconds<>, your next attack deals <#ff9028ff>bonus physical damage<> equal to <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>, granting <#60e84dff>${hsBonusHpPct}%<> of that damage as permanent <#60e84dff>bonus health<>."
$i18n.en.radiant_heartsteel.option = "Ironheart: Every <#e8a800ff>${rhsCd} seconds<>, your next attack deals <#ff9028ff>bonus physical damage<> equal to <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>, granting <#60e84dff>${rhsBonusHpPct}%<> of that damage as permanent <#60e84dff>bonus health<>."
$i18n.en.spear_of_shojin.option = "Focused Will: Landing abilities on enemy champions grants <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>Attack Damage<> for <#e8a800ff>${sosDur} seconds<> (max ${sosStacks} stacks)."
$i18n.en.radiant_spear_of_shojin.option = "Focused Will: Landing abilities on enemy champions grants <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>Attack Damage<> for <#e8a800ff>${rsosDur} seconds<> (max ${rsosStacks} stacks)."

Write-Host "Done."
Write-Host "Updating Vietnamese text."

$i18n.vi.executioners_calling.option = "Đao phủ: Khi tấn công, gây <#d94c49ff>${execHeal}% giảm hồi máu<> trong <#e8a800ff>${execDur} giây<>."
$i18n.vi.oblivion_orb.option = "Vết thương chí mạng: Gây <#a974ffff>sát thương phép<> lên tướng địch <#d94c49ff>giảm hồi máu của chúng ${ooHeal}%<> trong <#e8a800ff>${ooDur} giây<>."
$i18n.vi.morellonomicon.option = "Vết thương chí mạng: Gây <#a974ffff>sát thương phép<> lên tướng địch <#d94c49ff>giảm hồi máu của chúng ${morHeal}%<> trong <#e8a800ff>${morDur} giây<>."
$i18n.vi.radiant_morellonomicon.option = "Vết thương chí mạng: Gây <#a974ffff>sát thương phép<> lên tướng địch <#d94c49ff>giảm hồi máu của chúng ${rmorHeal}%<> trong <#e8a800ff>${rmorDur} giây<>."
$i18n.vi.overlords_bloodmail.option = "Bạo Ngược: Nhận <#ff9028ff>thêm<> <$adIcon> <#ff9028ff>SMCK<> tương ứng <#60e84dff>${obmAtk}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân."
$i18n.vi.radiant_overlords_bloodmail.option = "Bạo Ngược: Nhận <#ff9028ff>thêm<> <$adIcon> <#ff9028ff>SMCK<> tương ứng <#60e84dff>${robmAtk}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân."
$i18n.vi.night_harvester.option = "Cắt Xé Linh Hồn: Gây sát thương lên tướng địch sẽ gây thêm <#a974ffff>${nhFlat}<> + <#a974ffff>${nhApPct}%<> <$apIcon> <#a974ffff>SMPT<> dưới dạng <#a974ffff>sát thương phép<> và nhận <#ffffffff>${nhMs}%<> <$speedIcon> <#ffffffff>tốc độ di chuyển<> trong <#e8a800ff>${nhDur} giây<> (hồi chiêu ${nhCd} giây mỗi mục tiêu)."
$i18n.vi.radiant_night_harvester.option = "Cắt Xé Linh Hồn: Gây sát thương lên tướng địch sẽ gây thêm <#a974ffff>${rnhFlat}<> + <#a974ffff>${rnhApPct}%<> <$apIcon> <#a974ffff>SMPT<> dưới dạng <#a974ffff>sát thương phép<> và nhận <#ffffffff>${rnhMs}%<> <$speedIcon> <#ffffffff>tốc độ di chuyển<> trong <#e8a800ff>${rnhDur} giây<> (hồi chiêu ${rnhCd} giây mỗi mục tiêu)."
$i18n.vi.protectors_vow.option = "Kính Sợ: Nhận <#60e84dff>máu cộng thêm<> tương ứng <#60e84dff>${pvFlat}<> + <#ffdd8eff>${pvArmorPct}%<> <$armorIcon> <#ffdd8eff>Giáp<> của bạn."
$i18n.vi.radiant_protectors_vow.option = "Kính Sợ: Nhận <#60e84dff>máu cộng thêm<> tương ứng <#60e84dff>${rpvFlat}<> + <#ffdd8eff>${rpvArmorPct}%<> <$armorIcon> <#ffdd8eff>Giáp<> của bạn."
$i18n.vi.nashors_tooth.option = "Vết cắn Icathian: Khi tấn công, gây <#a974ffff>thêm sát thương phép<> tương ứng <#a974ffff>${ntFlat}<> + <#a974ffff>${ntApPct}%<> <$apIcon> <#a974ffff>SMPT<>."
$i18n.vi.radiant_nashors_tooth.option = "Vết cắn Icathian: Khi tấn công, gây <#a974ffff>thêm sát thương phép<> tương ứng <#a974ffff>${rntFlat}<> + <#a974ffff>${rntApPct}%<> <$apIcon> <#a974ffff>SMPT<>."
$i18n.vi.riftmaker.option = "Dung hòa: Kĩ năng trúng đích sẽ tăng <$apIcon> <#a974ffff>SMPT<> tương ứng <#60e84dff>${rmHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân trong <#e8a800ff>${rmDur} giây<> (tối đa ${rmStacks} cộng dồn)."
$i18n.vi.radiant_riftmaker.option = "Dung hòa: Kĩ năng trúng đích sẽ tăng <$apIcon> <#a974ffff>SMPT<> tương ứng <#60e84dff>${rrmHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân trong <#e8a800ff>${rrmDur} giây<> (tối đa ${rrmStacks} cộng dồn)."
$i18n.vi.mortal_reminder.option = "Đao phủ: Khi tấn công, gây <#d94c49ff>${mrHeal}% giảm hồi máu<> trong <#e8a800ff>${mrDur} giây<>."
$i18n.vi.radiant_mortal_reminder.option = "Đao phủ: Khi tấn công, gây <#d94c49ff>${rmrHeal}% giảm hồi máu<> trong <#e8a800ff>${rmrDur} giây<>."
$i18n.vi.jaksho_the_protean.option = "Kiên cường: Nhận sát thương từ tướng địch sẽ nhận thêm <#ffdd8eff>${jakDefMult}% <$armorIcon> giáp<> và <#88ccffff>${jakMrMult}% <$mrIcon> kháng phép<> trong <#e8a800ff>${jakDur} giây<> (tối đa ${jakStacks} cộng dồn)."
$i18n.vi.radiant_jaksho_the_protean.option = "Kiên cường: Nhận sát thương từ tướng địch sẽ nhận thêm <#ffdd8eff>${rjakDefMult}% <$armorIcon> giáp<> và <#88ccffff>${rjakMrMult}% <$mrIcon> kháng phép<> trong <#e8a800ff>${rjakDur} giây<> (tối đa ${rjakStacks} cộng dồn)."
$i18n.vi.frozen_mallet.option = "Băng kết: Khi tấn công, gây <#d94c49ff>${fmSlow}% kiệt sức <> trong <#e8a800ff>${fmDur} giây<>."
$i18n.vi.radiant_frozen_mallet.option = "Băng kết: Khi tấn công, gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân và gây <#d94c49ff>${rfmSlow}% kiệt sức <> trong <#e8a800ff>${rfmDur} giây<>."
$i18n.vi.experimental_hexplate.option = "Tăng tốc: Nhận <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>giảm thời gian hồi chiêu<> cuối."
$i18n.vi.radiant_experimental_hexplate.option = "Tăng tốc: Nhận <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>giảm thời gian hồi chiêu<> cuối."
$i18n.vi.guinsoos_rageblade.option = "Thịnh nộ: Tấn công thường gây ra thêm <#a974ffff>${gbDmg} sát thương phép thuật<>.`n`nDồn dập: Khi tấn công, nhận <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>tốc độ đánh<> trong <#e8a800ff>${gbDur} giây<> (tối đa ${gbStacks} cộng dồn)."
$i18n.vi.radiant_guinsoos_rageblade.option = "Thịnh nộ: Tấn công thường gây ra thêm <#a974ffff>${rgbDmg} sát thương phép thuật<>.`n`nDồn dập: Khi tấn công, nhận <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>tốc độ đánh<> trong <#e8a800ff>${rgbDur} giây<> (tối đa ${rgbStacks} cộng dồn)."
$i18n.vi.blackfire_torch.option = "Tội ác: Kĩ năng trúng sẽ tăng <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>SMPT<> trong <#e8a800ff>${bftDur} giây<> (tối đa ${bftStacks} cộng dồn)."
$i18n.vi.radiant_blackfire_torch.option = "Tội ác: Kĩ năng trúng sẽ tăng <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>SMPT<> trong <#e8a800ff>${rbftDur} giây<> (tối đa ${rbftStacks} cộng dồn)."
$i18n.vi.blade_of_the_ruined_king.option = "Nanh vuốt sương mù: Khi tấn công, gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#d94c49ff>${borkPct}% máu hiện tại của mục tiêu<>. Tối đa <#ff9028ff>${borkCap} sát thương vật lí<> lên lính và quái vật."
$i18n.vi.radiant_blade_of_the_ruined_king.option = "Nanh vuốt sương mù: Khi tấn công, gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#d94c49ff>${rborkPct}% máu hiện tại của mục tiêu<>. Tối đa <#ff9028ff>${rborkCap} sát thương vật lí<> lên lính và quái vật."
$i18n.vi.deathblade.option = "Cường hóa: Tăng <$adIcon> <#ff9028ff>SMCK<> của bản thân thêm <#ff9028ff>${dbMult}%<>."
$i18n.vi.radiant_deathblade.option = "Cường hóa: Tăng <$adIcon> <#ff9028ff>SMCK<> của bản thân thêm <#ff9028ff>${rdbMult}%<>."
$i18n.vi.deaths_dance.option = "Phớt Lờ Đau Đớn: <#e8a800ff>${ddDelay}%<> sát thương nhận vào được tích trữ và gây lại cho bạn theo thời gian dưới dạng <#d94c49ff>sát thương vật lí<> (tối đa <#60e84dff>${ddBurnCap}%<> <$hpIcon> <#60e84dff>máu tối đa<> mỗi giây).`n`nCự Tuyệt: Khi hạ gục, lượng sát thương tích trữ bị xóa và bạn <#60e84dff>hồi<> <#60e84dff>${ddHeal}%<> <#60e84dff>máu đã mất<>."
$i18n.vi.radiant_deaths_dance.option = "Phớt Lờ Đau Đớn: <#e8a800ff>${rddDelay}%<> sát thương nhận vào được tích trữ và gây lại cho bạn theo thời gian dưới dạng <#d94c49ff>sát thương vật lí<> (tối đa <#60e84dff>${rddBurnCap}%<> <$hpIcon> <#60e84dff>máu tối đa<> mỗi giây).`n`nCự Tuyệt: Khi hạ gục, lượng sát thương tích trữ bị xóa và bạn <#60e84dff>hồi<> <#60e84dff>${rddHeal}%<> <#60e84dff>máu đã mất<>."
$i18n.vi.rabadons_deathcap.option = "Hạt nhân: Tăng <$apIcon> <#a974ffff>SMPT<> của bản thân thêm <#a974ffff>${rabMult}%<>."
$i18n.vi.radiant_rabadons_deathcap.option = "Hạt nhân: Tăng <$apIcon> <#a974ffff>SMPT<> của bản thân thêm <#a974ffff>${radRabMult}%<>."

$mbIllusionVi = "Ảo ảnh: Nhận <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Lực Thích Ứng<>. Với mỗi <$forceIcon> <#d48294ff>Lực Thích Ứng<> tăng <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>SMCK<> hoặc <#a974ffff>1<> <$apIcon> <#a974ffff>SMPT<>, tùy vào bên nào cao hơn.`n`nMờ ảo: Khi kết liễu tướng địch, nhận <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>tốc độ di chuyển<> trong <#e8a800ff>{2} giây<>."
$i18n.vi.mirage_blade.option = $mbIllusionVi -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.vi.radiant_mirage_blade.option = $mbIllusionVi -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$i18n.vi.spirit_visage.option = "Sức sống: Tăng khả năng <#60e84dff>nhận hồi máu<> thêm <#60e84dff>${svHeal}%<>."
$i18n.vi.radiant_spirit_visage.option = "Sức sống: Tăng khả năng <#60e84dff>nhận hồi máu<> thêm <#60e84dff>${rsvHeal}%<>."
$i18n.vi.unending_despair.option = "Thống khổ: Kĩ năng trúng bạn sẽ hồi máu một lượng tương ứng <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> cho bản thân."
$i18n.vi.radiant_unending_despair.option = "Thống khổ: Kĩ năng trúng bạn sẽ hồi máu một lượng tương ứng <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> cho bản thân."
$i18n.vi.protoplasm_harness.option = "Vững chãi: Máu rơi xuống dưới <#d94c49ff>${phThreshold}%<> sẽ nhận <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, được xem là <#60e84dff>máu cộng thêm<> trong <#e8a800ff>${phDur} giây<> và <#60e84dff>hồi máu<> bằng phân nữa lượng đó (trong ${phCd} giây.)."
$i18n.vi.radiant_protoplasm_harness.option = "Vững chãi: Máu rơi xuống dưới <#d94c49ff>${rphThreshold}%<> sẽ nhận <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, được xem là <#60e84dff>máu cộng thêm<> trong <#e8a800ff>${rphDur} giây<> và <#60e84dff>hồi máu<> bằng phân nữa lượng đó (trong ${rphCd} giây.)."
$i18n.vi.terminus.option = "Đối lập: Khi tấn công, nhận luân phiên <#ffdd8eff>${tPen}% <$armorPenIcon> xuyên giáp<> hoặc <#88ccffff>${tPen}% <$magicPenIcon> xuyên kháng phép<> trong <#e8a800ff>${tDur} giây<> (tối đa ${tStacks} cộng dồn cho mỗi loại)."
$i18n.vi.radiant_terminus.option = "Đối lập: Khi tấn công, nhận luân phiên <#ffdd8eff>${rtPen}% <$armorPenIcon> xuyên giáp<> hoặc <#88ccffff>${rtPen}% <$magicPenIcon> xuyên kháng phép<> trong <#e8a800ff>${rtDur} giây<> (tối đa ${rtStacks} cộng dồn cho mỗi loại)."
$i18n.vi.collector.option = "Về Với Cát Bụi: Gây sát thương lên tướng địch dưới <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>Máu<> sẽ lập tức <#d94c49ff>kết liễu<> chúng."
$i18n.vi.radiant_collector.option = "Về Với Cát Bụi: Gây sát thương lên tướng địch dưới <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>Máu<> sẽ lập tức <#d94c49ff>kết liễu<> chúng."
$i18n.vi.heartsteel.option = "Trái Tim Sắt Đá: Mỗi <#e8a800ff>${hsCd} giây<>, đòn đánh tiếp theo của bạn gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, đồng thời nhận vĩnh viễn <#60e84dff>${hsBonusHpPct}%<> sát thương đó dưới dạng <#60e84dff>máu cộng thêm<>."
$i18n.vi.radiant_heartsteel.option = "Trái Tim Sắt Đá: Mỗi <#e8a800ff>${rhsCd} giây<>, đòn đánh tiếp theo của bạn gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, đồng thời nhận vĩnh viễn <#60e84dff>${rhsBonusHpPct}%<> sát thương đó dưới dạng <#60e84dff>máu cộng thêm<>."
$i18n.vi.spear_of_shojin.option = "Ý Chí Tập Trung: Kĩ năng trúng tướng địch sẽ tăng <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>SMCK<> trong <#e8a800ff>${sosDur} giây<> (tối đa ${sosStacks} cộng dồn)."
$i18n.vi.radiant_spear_of_shojin.option = "Ý Chí Tập Trung: Kĩ năng trúng tướng địch sẽ tăng <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>SMCK<> trong <#e8a800ff>${rsosDur} giây<> (tối đa ${rsosStacks} cộng dồn)."

Write-Host "Done."
Write-Host "Updating Chinese (Simplified) text."

$i18n.'zh-hans'.executioners_calling.option = "重伤：普通攻击使目标的<#d94c49ff>治疗效果降低${execHeal}%<>，持续 <#e8a800ff>${execDur}秒<>。"
$i18n.'zh-hans'.oblivion_orb.option = "重伤：对敌方英雄造成<#a974ffff>魔法伤害<>会使其<#d94c49ff>治疗效果降低${ooHeal}%<>，持续 <#e8a800ff>${ooDur}秒<>。"
$i18n.'zh-hans'.morellonomicon.option = "重伤：对敌方英雄造成<#a974ffff>魔法伤害<>会使其<#d94c49ff>治疗效果降低${morHeal}%<>，持续 <#e8a800ff>${morDur}秒<>。"
$i18n.'zh-hans'.radiant_morellonomicon.option = "重伤：对敌方英雄造成<#a974ffff>魔法伤害<>会使其<#d94c49ff>治疗效果降低${rmorHeal}%<>，持续 <#e8a800ff>${rmorDur}秒<>。"
$i18n.'zh-hans'.overlords_bloodmail.option = "暴政：获得相当于你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${obmAtk}%<> 的<#ff9028ff>额外<> <$adIcon> <#ff9028ff>攻击力<>。"
$i18n.'zh-hans'.radiant_overlords_bloodmail.option = "暴政：获得相当于你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${robmAtk}%<> 的<#ff9028ff>额外<> <$adIcon> <#ff9028ff>攻击力<>。"
$i18n.'zh-hans'.night_harvester.option = "裂魂：对敌方英雄造成伤害时，额外造成 <#a974ffff>${nhFlat}<> + <#a974ffff>${nhApPct}%<> <$apIcon> <#a974ffff>法术强度<> 的<#a974ffff>额外魔法伤害<>，并获得 <#ffffffff>${nhMs}%<> <$speedIcon> <#ffffffff>移动速度<>，持续 <#e8a800ff>${nhDur}秒<>（每个目标冷却${nhCd}秒）。"
$i18n.'zh-hans'.radiant_night_harvester.option = "裂魂：对敌方英雄造成伤害时，额外造成 <#a974ffff>${rnhFlat}<> + <#a974ffff>${rnhApPct}%<> <$apIcon> <#a974ffff>法术强度<> 的<#a974ffff>额外魔法伤害<>，并获得 <#ffffffff>${rnhMs}%<> <$speedIcon> <#ffffffff>移动速度<>，持续 <#e8a800ff>${rnhDur}秒<>（每个目标冷却${rnhCd}秒）。"
$i18n.'zh-hans'.protectors_vow.option = "敬畏：获得 <#60e84dff>${pvFlat}<> + 你的 <$armorIcon> <#ffdd8eff>护甲<> 的 <#ffdd8eff>${pvArmorPct}%<> 作为<#60e84dff>额外生命值<>。"
$i18n.'zh-hans'.radiant_protectors_vow.option = "敬畏：获得 <#60e84dff>${rpvFlat}<> + 你的 <$armorIcon> <#ffdd8eff>护甲<> 的 <#ffdd8eff>${rpvArmorPct}%<> 作为<#60e84dff>额外生命值<>。"
$i18n.'zh-hans'.nashors_tooth.option = "艾卡西亚之咬：普通攻击造成 <#a974ffff>${ntFlat}<> + <$apIcon> <#a974ffff>法术强度<>的 <#a974ffff>${ntApPct}%<> 的<#a974ffff>额外魔法伤害<>。"
$i18n.'zh-hans'.radiant_nashors_tooth.option = "艾卡西亚之咬：普通攻击造成 <#a974ffff>${rntFlat}<> + <$apIcon> <#a974ffff>法术强度<>的 <#a974ffff>${rntApPct}%<> 的<#a974ffff>额外魔法伤害<>。"
$i18n.'zh-hans'.riftmaker.option = "虚空灌注：技能命中时，获得相当于你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rmHpPct}%<> 的 <$apIcon> <#a974ffff>法术强度<>，持续 <#e8a800ff>${rmDur}秒<>（最多叠加${rmStacks}层）。"
$i18n.'zh-hans'.radiant_riftmaker.option = "虚空灌注：技能命中时，获得相当于你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rrmHpPct}%<> 的 <$apIcon> <#a974ffff>法术强度<>，持续 <#e8a800ff>${rrmDur}秒<>（最多叠加${rrmStacks}层）。"
$i18n.'zh-hans'.mortal_reminder.option = "重伤：普通攻击使目标的<#d94c49ff>治疗效果降低${mrHeal}%<>，持续 <#e8a800ff>${mrDur}秒<>。"
$i18n.'zh-hans'.radiant_mortal_reminder.option = "重伤：普通攻击使目标的<#d94c49ff>治疗效果降低${rmrHeal}%<>，持续 <#e8a800ff>${rmrDur}秒<>。"
$i18n.'zh-hans'.jaksho_the_protean.option = "复原力：受到敌方英雄伤害时，获得 <#ffdd8eff>${jakDefMult}% <$armorIcon> 护甲<>和 <#88ccffff>${jakMrMult}% <$mrIcon> 魔法抗性<>，持续 <#e8a800ff>${jakDur}秒<>（最多叠加${jakStacks}层）。"
$i18n.'zh-hans'.radiant_jaksho_the_protean.option = "复原力：受到敌方英雄伤害时，获得 <#ffdd8eff>${rjakDefMult}% <$armorIcon> 护甲<>和 <#88ccffff>${rjakMrMult}% <$mrIcon> 魔法抗性<>，持续 <#e8a800ff>${rjakDur}秒<>（最多叠加${rjakStacks}层）。"
$i18n.'zh-hans'.frozen_mallet.option = "冰寒：普通攻击会施加 <#d94c49ff>${fmSlow}%减速<>，持续 <#e8a800ff>${fmDur}秒<>。"
$i18n.'zh-hans'.radiant_frozen_mallet.option = "冰寒：普通攻击造成 <#ff9028ff>额外物理伤害<>，数值为 <#ff9028ff>${rfmFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rfmHpPct}%<>，并施加 <#d94c49ff>${rfmSlow}%减速<>，持续 <#e8a800ff>${rfmDur}秒<>。"
$i18n.'zh-hans'.experimental_hexplate.option = "过载：终极技能获得 <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>冷却缩减<>。"
$i18n.'zh-hans'.radiant_experimental_hexplate.option = "过载：终极技能获得 <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>冷却缩减<>。"
$i18n.'zh-hans'.guinsoos_rageblade.option = "怨怒：普通攻击造成 <#a974ffff>${gbDmg}点额外魔法伤害<>。`n`n沸腾打击：普通攻击时获得 <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>攻击速度<>，持续 <#e8a800ff>${gbDur}秒<>（最多叠加${gbStacks}层）。"
$i18n.'zh-hans'.radiant_guinsoos_rageblade.option = "怨怒：普通攻击造成 <#a974ffff>${rgbDmg}点额外魔法伤害<>。`n`n沸腾打击：普通攻击时获得 <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>攻击速度<>，持续 <#e8a800ff>${rgbDur}秒<>（最多叠加${rgbStacks}层）。"
$i18n.'zh-hans'.blackfire_torch.option = "邪焰：技能命中时获得 <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>法术强度<>，持续 <#e8a800ff>${bftDur}秒<>（最多叠加${bftStacks}层）。"
$i18n.'zh-hans'.radiant_blackfire_torch.option = "邪焰：技能命中时获得 <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>法术强度<>，持续 <#e8a800ff>${rbftDur}秒<>（最多叠加${rbftStacks}层）。"
$i18n.'zh-hans'.blade_of_the_ruined_king.option = "雾之锋：普通攻击造成相当于目标<#d94c49ff>当前生命值${borkPct}%<>的<#ff9028ff>额外物理伤害<>。对小兵和野怪最多造成 <#ff9028ff>${borkCap}点物理伤害<>。"
$i18n.'zh-hans'.radiant_blade_of_the_ruined_king.option = "雾之锋：普通攻击造成相当于目标<#d94c49ff>当前生命值${rborkPct}%<>的<#ff9028ff>额外物理伤害<>。对小兵和野怪最多造成 <#ff9028ff>${rborkCap}点物理伤害<>。"
$i18n.'zh-hans'.deathblade.option = "顶峰：你的总 <$adIcon> <#ff9028ff>攻击力<>提升 <#ff9028ff>${dbMult}%<>。"
$i18n.'zh-hans'.radiant_deathblade.option = "顶峰：你的总 <$adIcon> <#ff9028ff>攻击力<>提升 <#ff9028ff>${rdbMult}%<>。"
$i18n.'zh-hans'.deaths_dance.option = "无视痛苦：受到伤害的 <#e8a800ff>${ddDelay}%<> 会被储存，并随时间作为 <#d94c49ff>额外物理伤害<> 返还给你（每秒最多为你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${ddBurnCap}%<>）。`n`n拒止：击杀时，储存的伤害被清除，并为你回复 <#60e84dff>${ddHeal}%<> <#60e84dff>已损失生命值<>。"
$i18n.'zh-hans'.radiant_deaths_dance.option = "无视痛苦：受到伤害的 <#e8a800ff>${rddDelay}%<> 会被储存，并随时间作为 <#d94c49ff>额外物理伤害<> 返还给你（每秒最多为你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rddBurnCap}%<>）。`n`n拒止：击杀时，储存的伤害被清除，并为你回复 <#60e84dff>${rddHeal}%<> <#60e84dff>已损失生命值<>。"
$i18n.'zh-hans'.rabadons_deathcap.option = "魔法乐章：你的总 <$apIcon> <#a974ffff>法术强度<>提升 <#a974ffff>${rabMult}%<>。"
$i18n.'zh-hans'.radiant_rabadons_deathcap.option = "魔法乐章：你的总 <$apIcon> <#a974ffff>法术强度<>提升 <#a974ffff>${radRabMult}%<>。"

$mbIllusionZh = "幻象：获得 <#d48294ff>{0}<> <$forceIcon> <#d48294ff>自适应之力<>。每点 <$forceIcon> <#d48294ff>自适应之力<>会根据较高者提供 <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>攻击力<>或 <#a974ffff>1<> <$apIcon> <#a974ffff>法术强度<>。`n`n模糊：击杀时获得 <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>移动速度<>，持续 <#e8a800ff>{2}秒<>。"
$i18n.'zh-hans'.mirage_blade.option = $mbIllusionZh -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.'zh-hans'.radiant_mirage_blade.option = $mbIllusionZh -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$i18n.'zh-hans'.spirit_visage.option = "无拘活力：受到的所有<#60e84dff>治疗效果<>提升 <#60e84dff>${svHeal}%<>。"
$i18n.'zh-hans'.radiant_spirit_visage.option = "无拘活力：受到的所有<#60e84dff>治疗效果<>提升 <#60e84dff>${rsvHeal}%<>。"
$i18n.'zh-hans'.unending_despair.option = "苦楚：技能命中时，回复 <#60e84dff>${udFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${udHpPct}%<> 的生命值。"
$i18n.'zh-hans'.radiant_unending_despair.option = "苦楚：技能命中时，回复 <#60e84dff>${rudFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rudHpPct}%<> 的生命值。"
$i18n.'zh-hans'.protoplasm_harness.option = "筑防：<#d94c49ff>生命值降至${phThreshold}%以下<>时，获得 <#60e84dff>${phFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${phHpPct}%<> 作为<#60e84dff>额外生命值<>，持续 <#e8a800ff>${phDur}秒<>，并<#60e84dff>回复<>该数值一半的生命值（冷却时间${phCd}秒）。"
$i18n.'zh-hans'.radiant_protoplasm_harness.option = "筑防：<#d94c49ff>生命值降至${rphThreshold}%以下<>时，获得 <#60e84dff>${rphFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rphHpPct}%<> 作为<#60e84dff>额外生命值<>，持续 <#e8a800ff>${rphDur}秒<>，并<#60e84dff>回复<>该数值一半的生命值（冷却时间${rphCd}秒）。"
$i18n.'zh-hans'.terminus.option = "交相：普通攻击会交替获得 <#ffdd8eff>${tPen}% <$armorPenIcon> 穿甲<>或 <#88ccffff>${tPen}% <$magicPenIcon> 法术穿透<>，持续 <#e8a800ff>${tDur}秒<>（每种最多叠加${tStacks}层）。"
$i18n.'zh-hans'.radiant_terminus.option = "交相：普通攻击会交替获得 <#ffdd8eff>${rtPen}% <$armorPenIcon> 穿甲<>或 <#88ccffff>${rtPen}% <$magicPenIcon> 法术穿透<>，持续 <#e8a800ff>${rtDur}秒<>（每种最多叠加${rtStacks}层）。"
$i18n.'zh-hans'.collector.option = "死：对生命值低于 <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>最大生命值<> 的敌方英雄造成伤害时，<#d94c49ff>处决<>目标。"
$i18n.'zh-hans'.radiant_collector.option = "死：对生命值低于 <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#60e84dff>最大生命值<> 的敌方英雄造成伤害时，<#d94c49ff>处决<>目标。"
$i18n.'zh-hans'.heartsteel.option = "铁石心肠：每隔 <#e8a800ff>${hsCd}秒<>，你的下一次普通攻击造成 <#ff9028ff>额外物理伤害<>，数值为 <#ff9028ff>${hsFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${hsHpPct}%<>，并永久获得该伤害 <#60e84dff>${hsBonusHpPct}%<> 作为 <#60e84dff>额外生命值<>。"
$i18n.'zh-hans'.radiant_heartsteel.option = "铁石心肠：每隔 <#e8a800ff>${rhsCd}秒<>，你的下一次普通攻击造成 <#ff9028ff>额外物理伤害<>，数值为 <#ff9028ff>${rhsFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rhsHpPct}%<>，并永久获得该伤害 <#60e84dff>${rhsBonusHpPct}%<> 作为 <#60e84dff>额外生命值<>。"
$i18n.'zh-hans'.spear_of_shojin.option = "专注意志：技能命中敌方英雄时获得 <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>攻击力<>，持续 <#e8a800ff>${sosDur}秒<>（最多叠加${sosStacks}层）。"
$i18n.'zh-hans'.radiant_spear_of_shojin.option = "专注意志：技能命中敌方英雄时获得 <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>攻击力<>，持续 <#e8a800ff>${rsosDur}秒<>（最多叠加${rsosStacks}层）。"

Write-Host "Done."
Write-Host "Updating Portuguese (Brazil) text."

$i18n.'pt-BR'.executioners_calling.option = "Executor: Ataques aplicam <#d94c49ff>${execHeal}% de Redução de Cura<> for <#e8a800ff>${execDur} segundos<>."
$i18n.'pt-BR'.oblivion_orb.option = "Ferimentos Graves: Causar <#a974ffff>dano mágico<> a um campeão inimigo <#d94c49ff>reduz a cura dele em ${ooHeal}%<> por <#e8a800ff>${ooDur} segundos<>."
$i18n.'pt-BR'.morellonomicon.option = "Ferimentos Graves: Causar <#a974ffff>dano mágico<> a um campeão inimigo <#d94c49ff>reduz a cura dele em ${morHeal}%<> por <#e8a800ff>${morDur} segundos<>."
$i18n.'pt-BR'.radiant_morellonomicon.option = "Ferimentos Graves: Causar <#a974ffff>dano mágico<> a um campeão inimigo <#d94c49ff>reduz a cura dele em ${rmorHeal}%<> por <#e8a800ff>${rmorDur} segundos<>."
$i18n.'pt-BR'.overlords_bloodmail.option = "Tirania: Ganha <#ff9028ff>bônus<> de <$adIcon> <#ff9028ff>Dano de Ataque<> igual a <#60e84dff>${obmAtk}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>."
$i18n.'pt-BR'.radiant_overlords_bloodmail.option = "Tirania: Ganha <#ff9028ff>bônus<> de <$adIcon> <#ff9028ff>Dano de Ataque<> igual a <#60e84dff>${robmAtk}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>."
$i18n.'pt-BR'.night_harvester.option = "Dilacerar Almas: Causar dano a um campeão inimigo causa <#a974ffff>${nhFlat}<> + <#a974ffff>${nhApPct}%<> de <$apIcon> <#a974ffff>Poder de Habilidade<> como <#a974ffff>dano mágico bônus<> e concede <#ffffffff>${nhMs}%<> de <$speedIcon> <#ffffffff>Velocidade de Movimento<> por <#e8a800ff>${nhDur} segundos<> (recarga de ${nhCd} segundos por alvo)."
$i18n.'pt-BR'.radiant_night_harvester.option = "Dilacerar Almas: Causar dano a um campeão inimigo causa <#a974ffff>${rnhFlat}<> + <#a974ffff>${rnhApPct}%<> de <$apIcon> <#a974ffff>Poder de Habilidade<> como <#a974ffff>dano mágico bônus<> e concede <#ffffffff>${rnhMs}%<> de <$speedIcon> <#ffffffff>Velocidade de Movimento<> por <#e8a800ff>${rnhDur} segundos<> (recarga de ${rnhCd} segundos por alvo)."
$i18n.'pt-BR'.protectors_vow.option = "Temor: Ganha <#60e84dff>vida bônus<> igual a <#60e84dff>${pvFlat}<> + <#ffdd8eff>${pvArmorPct}%<> da sua <$armorIcon> <#ffdd8eff>Armadura<>."
$i18n.'pt-BR'.radiant_protectors_vow.option = "Temor: Ganha <#60e84dff>vida bônus<> igual a <#60e84dff>${rpvFlat}<> + <#ffdd8eff>${rpvArmorPct}%<> da sua <$armorIcon> <#ffdd8eff>Armadura<>."
$i18n.'pt-BR'.nashors_tooth.option = "Mordida Icathiana: Ataques causam <#a974ffff>${ntFlat}<> + <#a974ffff>${ntApPct}%<> <$apIcon> <#a974ffff>Poder de Habilidade<> como <#a974ffff>dano mágico bônus<>."
$i18n.'pt-BR'.radiant_nashors_tooth.option = "Mordida Icathiana: Ataques causam <#a974ffff>${rntFlat}<> + <#a974ffff>${rntApPct}%<> <$apIcon> <#a974ffff>Poder de Habilidade<> como <#a974ffff>dano mágico bônus<>."
$i18n.'pt-BR'.riftmaker.option = "Corrupção: Suas mágias te dão <$apIcon> <#a974ffff>Poder de Habilidade<> igual a <#60e84dff>${rmHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por <#e8a800ff>${rmDur} segundos<> (acumula ${rmStacks}x)."
$i18n.'pt-BR'.radiant_riftmaker.option = "Corrupção: Suas mágias te dão <$apIcon> <#a974ffff>Poder de Habilidade<> igual a <#60e84dff>${rrmHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por <#e8a800ff>${rrmDur} segundos<> (acumula ${rrmStacks}x)."
$i18n.'pt-BR'.mortal_reminder.option = "Executor: Ataques aplicam <#d94c49ff>${mrHeal}% de Redução de Cura<> for <#e8a800ff>${mrDur} segundos<>."
$i18n.'pt-BR'.radiant_mortal_reminder.option = "Executor: Ataques aplicam <#d94c49ff>${rmrHeal}% de Redução de Cura<> for <#e8a800ff>${rmrDur} segundos<>."
$i18n.'pt-BR'.jaksho_the_protean.option = "Resiliência: Receber dano de um campeão inimigo concede <#ffdd8eff>${jakDefMult}% <$armorIcon> de Armadura<> e <#88ccffff>${jakMrMult}% <$mrIcon> de Resistência Mágica<> por <#e8a800ff>${jakDur} segundos<> (acumula ${jakStacks}x)."
$i18n.'pt-BR'.radiant_jaksho_the_protean.option = "Resiliência: Receber dano de um campeão inimigo concede <#ffdd8eff>${rjakDefMult}% <$armorIcon> de Armadura<> e <#88ccffff>${rjakMrMult}% <$mrIcon> de Resistência Mágica<> por <#e8a800ff>${rjakDur} segundos<> (acumula ${rjakStacks}x)."
$i18n.'pt-BR'.frozen_mallet.option = "Congelante: Seus ataques aplicam <#d94c49ff>${fmSlow}% de lentidão<> por <#e8a800ff>${fmDur} segundos<>."
$i18n.'pt-BR'.radiant_frozen_mallet.option = "Congelante: Seus ataques causam <#ff9028ff>dano físico<> igual a <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> e aplicam <#d94c49ff>${rfmSlow}% de lentidão<> por <#e8a800ff>${rfmDur} segundos<>."
$i18n.'pt-BR'.experimental_hexplate.option = "Hexcarregado: Recebe <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>Redução de Tempo de Recarga<> na sua habilidade ultimate."
$i18n.'pt-BR'.radiant_experimental_hexplate.option = "Hexcarregado: Recebe <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>Redução de Tempo de Recarga<> na sua habilidade ultimate."
$i18n.'pt-BR'.guinsoos_rageblade.option = "Ira: Ataques causam <#a974ffff>${gbDmg} de dano mágico adicional<>.`n`nFervendo: Ataques concedem <#ceff99ff>${gbSpeed}%<> de <$asIcon> <#ceff99ff>Velocidade de Ataque<> por <#e8a800ff>${gbDur} segundos<> (acumula ${gbStacks}x)."
$i18n.'pt-BR'.radiant_guinsoos_rageblade.option = "Ira: Ataques causam <#a974ffff>${rgbDmg} de dano mágico adicional<>.`n`nFervendo: Ataques concedem <#ceff99ff>${rgbSpeed}%<> de <$asIcon> <#ceff99ff>Velocidade de Ataque<> por <#e8a800ff>${rgbDur} segundos<> (acumula ${rgbStacks}x)."
$i18n.'pt-BR'.blackfire_torch.option = "Nefastidão: Suas mágias te dão <#a974ffff>${bftPower}<> <$apIcon> de <#a974ffff>Poder de Habilidade<> por <#e8a800ff>${bftDur} segundos<> (acumula ${bftStacks}x)."
$i18n.'pt-BR'.radiant_blackfire_torch.option = "Nefastidão: Suas mágias te dão <#a974ffff>${rbftPower}<> <$apIcon> de <#a974ffff>Poder de Habilidade<> por <#e8a800ff>${rbftDur} segundos<> (acumula ${rbftStacks}x)."
$i18n.'pt-BR'.blade_of_the_ruined_king.option = "Gume da Névoa: Ataques causam <#d94c49ff>${borkPct}% da Vida Atual do alvo<> como <#ff9028ff>dano físico<> adicional. (Máximo de <#ff9028ff>${borkCap}<> contra tropas e monstros.)"
$i18n.'pt-BR'.radiant_blade_of_the_ruined_king.option = "Gume da Névoa: Ataques causam <#d94c49ff>${rborkPct}% da Vida Atual do alvo<> como <#ff9028ff>dano físico<> adicional. (Máximo de <#ff9028ff>${rborkCap}<> contra tropas e monstros.)"
$i18n.'pt-BR'.deathblade.option = "Apex: Aumenta seu <$adIcon> <#ff9028ff>Dano de Ataque<> em <#ff9028ff>${dbMult}%<>."
$i18n.'pt-BR'.radiant_deathblade.option = "Apex: Aumenta seu <$adIcon> <#ff9028ff>Dano de Ataque<> em <#ff9028ff>${rdbMult}%<>."
$i18n.'pt-BR'.deaths_dance.option = "Ignorar a Dor: <#e8a800ff>${ddDelay}%<> do dano sofrido é armazenado e causado de volta a você ao longo do tempo como <#d94c49ff>dano físico<> (até <#60e84dff>${ddBurnCap}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por segundo).`n`nDesafiar: Ao abater, o dano armazenado é removido e você se <#60e84dff>cura<> em <#60e84dff>${ddHeal}%<> da sua <#60e84dff>vida perdida<>."
$i18n.'pt-BR'.radiant_deaths_dance.option = "Ignorar a Dor: <#e8a800ff>${rddDelay}%<> do dano sofrido é armazenado e causado de volta a você ao longo do tempo como <#d94c49ff>dano físico<> (até <#60e84dff>${rddBurnCap}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por segundo).`n`nDesafiar: Ao abater, o dano armazenado é removido e você se <#60e84dff>cura<> em <#60e84dff>${rddHeal}%<> da sua <#60e84dff>vida perdida<>."
$i18n.'pt-BR'.rabadons_deathcap.option = "Apogeu: Aumenta o <$apIcon> <#a974ffff>Poder de Habilidade<> total em <#a974ffff>${rabMult}%<>."
$i18n.'pt-BR'.radiant_rabadons_deathcap.option = "Apogeu: Aumenta o <$apIcon> <#a974ffff>Poder de Habilidade<> total em <#a974ffff>${radRabMult}%<>"

$mbIllusionPt = "Ilusão: Ganha <#d48294ff>{0}<> <$forceIcon> de <#d48294ff>Força Adaptativa<>. Cada <$forceIcon> <#d48294ff>Força Adaptativa<> garante <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Dano de Ataque<> ou <#a974ffff>1<> <$apIcon> <#a974ffff>Poder de Habilidade<>, dependendo de qual é maior.`n`nBorrão: Abates concedem <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>Velocidade de Movimento<> por <#e8a800ff>{2} segundos<>."
$i18n.'pt-BR'.mirage_blade.option = $mbIllusionPt -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.'pt-BR'.radiant_mirage_blade.option = $mbIllusionPt -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$i18n.'pt-BR'.spirit_visage.option = "Vitalidade: <#60e84dff>Curas recebidas<> são aumentadas em <#60e84dff>${svHeal}%<>."
$i18n.'pt-BR'.radiant_spirit_visage.option = "Vitalidade: <#60e84dff>Curas recebidas<> são aumentadas em <#60e84dff>${rsvHeal}%<>."
$i18n.'pt-BR'.unending_despair.option = "Angústia: Suas mágias te curam em <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>."
$i18n.'pt-BR'.radiant_unending_despair.option = "Angústia: Suas mágias te curam em <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>."
$i18n.'pt-BR'.protoplasm_harness.option = "Fortificação: Ao receber dano suficiente para reduzir sua <#d94c49ff>Vida a menos de ${phThreshold}%<> você, recebe <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por <#e8a800ff>${phDur} segundos<>, depois <#60e84dff>cura<> por metade desse valor (${phCd}s)."
$i18n.'pt-BR'.radiant_protoplasm_harness.option = "Fortificação: Ao receber dano suficiente para reduzir sua <#d94c49ff>Vida a menos de ${rphThreshold}%<> você, recebe <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por <#e8a800ff>${rphDur} segundos<>, depois <#60e84dff>cura<> por metade desse valor (${rphCd}s)."
$i18n.'pt-BR'.terminus.option = "Justaposição: Ataques alternam para conceder entre <#ffdd8eff>${tPen}% <$armorPenIcon> de Penetração de Armadura<> ou <#88ccffff>${tPen}% <$magicPenIcon> Penetração Mágica<> por <#e8a800ff>${tDur} segundos<>. (acumula ${tStacks}x cada)."
$i18n.'pt-BR'.radiant_terminus.option = "Justaposição: Ataques alternam para conceder entre <#ffdd8eff>${rtPen}% <$armorPenIcon> de Penetração de Armadura<> ou <#88ccffff>${rtPen}% <$magicPenIcon> Penetração Mágica<> por <#e8a800ff>${rtDur} segundos<>. (acumula ${rtStacks}x cada)."
$i18n.'pt-BR'.collector.option = "Morte: Causar dano a campeões inimigos com menos de <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>Vida Máxima<> os <#d94c49ff>executa<>."
$i18n.'pt-BR'.radiant_collector.option = "Morte: Causar dano a campeões inimigos com menos de <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#60e84dff>Vida Máxima<> os <#d94c49ff>executa<>."
$i18n.'pt-BR'.heartsteel.option = "Coração Forjado: A cada <#e8a800ff>${hsCd} segundos<>, seu próximo ataque causa <#ff9028ff>dano físico<> adicional igual a <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>, concedendo permanentemente <#60e84dff>${hsBonusHpPct}%<> desse dano como <#60e84dff>vida bônus<>."
$i18n.'pt-BR'.radiant_heartsteel.option = "Coração Forjado: A cada <#e8a800ff>${rhsCd} segundos<>, seu próximo ataque causa <#ff9028ff>dano físico<> adicional igual a <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>, concedendo permanentemente <#60e84dff>${rhsBonusHpPct}%<> desse dano como <#60e84dff>vida bônus<>."
$i18n.'pt-BR'.spear_of_shojin.option = "Vontade Focada: Acertar habilidades em campeões inimigos concede <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>Dano de Ataque<> por <#e8a800ff>${sosDur} segundos<> (acumula ${sosStacks}x)."
$i18n.'pt-BR'.radiant_spear_of_shojin.option = "Vontade Focada: Acertar habilidades em campeões inimigos concede <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>Dano de Ataque<> por <#e8a800ff>${rsosDur} segundos<> (acumula ${rsosStacks}x)."

Write-Host "Done."
Write-Host "Updating Russian text."

$i18n.ru.executioners_calling.option = "Палач: При атаке, <#d94c49ff>снижает лечение на ${execHeal}%<> на <#e8a800ff>${execDur} секунды<>."
$i18n.ru.oblivion_orb.option = "Тяжёлые раны: Нанесение <#a974ffff>магического урона<> вражескому чемпиону <#d94c49ff>снижает его лечение на ${ooHeal}%<> на <#e8a800ff>${ooDur} секунды<>."
$i18n.ru.morellonomicon.option = "Тяжёлые раны: Нанесение <#a974ffff>магического урона<> вражескому чемпиону <#d94c49ff>снижает его лечение на ${morHeal}%<> на <#e8a800ff>${morDur} секунды<>."
$i18n.ru.radiant_morellonomicon.option = "Тяжёлые раны: Нанесение <#a974ffff>магического урона<> вражескому чемпиону <#d94c49ff>снижает его лечение на ${rmorHeal}%<> на <#e8a800ff>${rmorDur} секунды<>."
$i18n.ru.overlords_bloodmail.option = "Тирания: Даёт <#ff9028ff>дополнительную<> <$adIcon> <#ff9028ff>Силу Атаки<> равную <#60e84dff>${obmAtk}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>."
$i18n.ru.radiant_overlords_bloodmail.option = "Тирания: Даёт <#ff9028ff>дополнительную<> <$adIcon> <#ff9028ff>Силу Атаки<> равную <#60e84dff>${robmAtk}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>."
$i18n.ru.night_harvester.option = "Разрыв души: Нанесение урона вражескому чемпиону наносит <#a974ffff>${nhFlat}<> + <#a974ffff>${nhApPct}%<> <$apIcon> <#a974ffff>Силы Умений<> как <#a974ffff>дополнительный магический урон<> и даёт <#ffffffff>${nhMs}%<> <$speedIcon> <#ffffffff>скорости передвижения<> на <#e8a800ff>${nhDur} секунды<> (перезарядка ${nhCd} секунд на каждую цель)."
$i18n.ru.radiant_night_harvester.option = "Разрыв души: Нанесение урона вражескому чемпиону наносит <#a974ffff>${rnhFlat}<> + <#a974ffff>${rnhApPct}%<> <$apIcon> <#a974ffff>Силы Умений<> как <#a974ffff>дополнительный магический урон<> и даёт <#ffffffff>${rnhMs}%<> <$speedIcon> <#ffffffff>скорости передвижения<> на <#e8a800ff>${rnhDur} секунды<> (перезарядка ${rnhCd} секунд на каждую цель)."
$i18n.ru.protectors_vow.option = "Трепет: Даёт <#60e84dff>дополнительное здоровье<> равное <#60e84dff>${pvFlat}<> + <#ffdd8eff>${pvArmorPct}%<> вашей <$armorIcon> <#ffdd8eff>брони<>."
$i18n.ru.radiant_protectors_vow.option = "Трепет: Даёт <#60e84dff>дополнительное здоровье<> равное <#60e84dff>${rpvFlat}<> + <#ffdd8eff>${rpvArmorPct}%<> вашей <$armorIcon> <#ffdd8eff>брони<>."
$i18n.ru.nashors_tooth.option = "Укус Икатии: При атаке наносит <#a974ffff>дополнительный магический урон<> равный <#a974ffff>${ntFlat}<> + <#a974ffff>${ntApPct}%<> <$apIcon> <#a974ffff>Силы Умений<>."
$i18n.ru.radiant_nashors_tooth.option = "Укус Икатии: При атаке наносит <#a974ffff>дополнительный магический урон<> равный <#a974ffff>${rntFlat}<> + <#a974ffff>${rntApPct}%<> <$apIcon> <#a974ffff>Силы Умений<>."
$i18n.ru.riftmaker.option = "Наполнение: Попадания умениями дают <$apIcon> <#a974ffff>Силу Умений<> равную <#60e84dff>${rmHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> на <#e8a800ff>${rmDur} секунд<> (макс. ${rmStacks} стака)."
$i18n.ru.radiant_riftmaker.option = "Наполнение: Попадания умениями дают <$apIcon> <#a974ffff>Силу Умений<> равную <#60e84dff>${rrmHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> на <#e8a800ff>${rrmDur} секунд<> (макс. ${rrmStacks} стака)."
$i18n.ru.mortal_reminder.option = "Палач: При атаке, <#d94c49ff>снижает лечение на ${mrHeal}%<> на <#e8a800ff>${mrDur} секунды<>."
$i18n.ru.radiant_mortal_reminder.option = "Палач: При атаке, <#d94c49ff>снижает лечение на ${rmrHeal}%<> на <#e8a800ff>${rmrDur} секунды<>."
$i18n.ru.jaksho_the_protean.option = "Стойкость: Получение урона от вражеского чемпиона даёт <#ffdd8eff>${jakDefMult}% <$armorIcon> брони<> и <#88ccffff>${jakMrMult}% <$mrIcon> сопротивления магии<> на <#e8a800ff>${jakDur} секунды<> (макс. ${jakStacks} стака)."
$i18n.ru.radiant_jaksho_the_protean.option = "Стойкость: Получение урона от вражеского чемпиона даёт <#ffdd8eff>${rjakDefMult}% <$armorIcon> брони<> и <#88ccffff>${rjakMrMult}% <$mrIcon> сопротивления магии<> на <#e8a800ff>${rjakDur} секунды<> (макс. ${rjakStacks} стака)."
$i18n.ru.frozen_mallet.option = "Обледенение: При атаке накладывает <#d94c49ff>замедление на ${fmSlow}%<> на <#e8a800ff>${fmDur} секунды<>."
$i18n.ru.radiant_frozen_mallet.option = "Обледенение: При атаке наносит <#ff9028ff>дополнительный физический урон<> равный <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> и накладывает <#d94c49ff>замедление на ${rfmSlow}%<> на <#e8a800ff>${rfmDur} секунды<>."
$i18n.ru.experimental_hexplate.option = "Перегрузка: Даёт <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>сокращения времени перезарядки<> вашего ультимейта."
$i18n.ru.radiant_experimental_hexplate.option = "Перегрузка: Даёт <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>сокращения времени перезарядки<> вашего ультимейта."
$i18n.ru.guinsoos_rageblade.option = "Гнев: Ваши базовые атаки наносят <#a974ffff>${gbDmg} дополнительного магического урона<>.`n`nЯростный Удар: При атаке даёт <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>скорости атаки<> на <#e8a800ff>${gbDur} секунды<> (макс. ${gbStacks} стака)."
$i18n.ru.radiant_guinsoos_rageblade.option = "Гнев: Ваши базовые атаки наносят <#a974ffff>${rgbDmg} дополнительного магического урона<>.`n`nЯростный Удар: При атаке даёт <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>скорости атаки<> на <#e8a800ff>${rgbDur} секунды<> (макс. ${rgbStacks} стака)."
$i18n.ru.blackfire_torch.option = "Злодейский: Попадания умениями дают <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>Силы Умений<> на <#e8a800ff>${bftDur} секунды<> (макс. ${bftStacks} стака)."
$i18n.ru.radiant_blackfire_torch.option = "Злодейский: Попадания умениями дают <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>Силы Умений<> на <#e8a800ff>${rbftDur} секунды<> (макс. ${rbftStacks} стака)."
$i18n.ru.blade_of_the_ruined_king.option = "Край тумана: При атаке наносит <#ff9028ff>дополнительный физический урон<> равный <#d94c49ff>${borkPct}% текущего здоровья цели<>. Наносит максимум <#ff9028ff>${borkCap} физического урона<> по миньонам и монстрам."
$i18n.ru.radiant_blade_of_the_ruined_king.option = "Край тумана: При атаке наносит <#ff9028ff>дополнительный физический урон<> равный <#d94c49ff>${rborkPct}% текущего здоровья цели<>. Наносит максимум <#ff9028ff>${rborkCap} физического урона<> по миньонам и монстрам."
$i18n.ru.deathblade.option = "Вершина: Увеличивает вашу общую <$adIcon> <#ff9028ff>Силу Атаки<> на <#ff9028ff>${dbMult}%<>."
$i18n.ru.radiant_deathblade.option = "Вершина: Увеличивает вашу общую <$adIcon> <#ff9028ff>Силу Атаки<> на <#ff9028ff>${rdbMult}%<>."
$i18n.ru.deaths_dance.option = "Игнорирование Боли: <#e8a800ff>${ddDelay}%<> получаемого урона вместо этого наносится с течением времени как <#d94c49ff>физический урон<> (до <#60e84dff>${ddBurnCap}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> в секунду).`n`nВызов: При убийстве снимает весь накопленный урон и <#60e84dff>восстанавливает<> <#60e84dff>${ddHeal}%<> от вашего <#60e84dff>потерянного здоровья<>."
$i18n.ru.radiant_deaths_dance.option = "Игнорирование Боли: <#e8a800ff>${rddDelay}%<> получаемого урона вместо этого наносится с течением времени как <#d94c49ff>физический урон<> (до <#60e84dff>${rddBurnCap}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> в секунду).`n`nВызов: При убийстве снимает весь накопленный урон и <#60e84dff>восстанавливает<> <#60e84dff>${rddHeal}%<> от вашего <#60e84dff>потерянного здоровья<>."
$i18n.ru.rabadons_deathcap.option = "Опус: Увеличивает вашу общую <$apIcon> <#a974ffff>Силу Умений<> на <#a974ffff>${rabMult}%<>."
$i18n.ru.radiant_rabadons_deathcap.option = "Опус: Увеличивает вашу общую <$apIcon> <#a974ffff>Силу Умений<> на <#a974ffff>${radRabMult}%<>."

$mbIllusionRu = "Иллюзия: Даёт <#d48294ff>{0}<> <$forceIcon> <#d48294ff>адаптивной силы<>. Каждая единица <$forceIcon> <#d48294ff>адаптивной силы<> даёт <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Силы атаки<> или <#a974ffff>1<> <$apIcon> <#a974ffff>Силы умений<>, в зависимости от того, что выше.`n`nРазымытие: При убийстве даёт <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>скорости передвижения<> на <#e8a800ff>{2} секунды<>."
$i18n.ru.mirage_blade.option = $mbIllusionRu -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.ru.radiant_mirage_blade.option = $mbIllusionRu -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$i18n.ru.spirit_visage.option = "Живительная Сила: Увеличивает всё <#60e84dff>получаемое лечение<> на <#60e84dff>${svHeal}%<>."
$i18n.ru.radiant_spirit_visage.option = "Живительная Сила: Увеличивает всё <#60e84dff>получаемое лечение<> на <#60e84dff>${rsvHeal}%<>."
$i18n.ru.unending_despair.option = "Страдание: Попадания умениями восстанавливают вам <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>."
$i18n.ru.radiant_unending_despair.option = "Страдание: Попадания умениями восстанавливают вам <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>."
$i18n.ru.protoplasm_harness.option = "Укрепление: При <#d94c49ff>падении здоровья ниже ${phThreshold}%<> даёт <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> как <#60e84dff>дополнительное здоровье<> на <#e8a800ff>${phDur} секунд<> и <#60e84dff>восстанавливает вам<> половину этого количества (перезарядка ${phCd} секунд)."
$i18n.ru.radiant_protoplasm_harness.option = "Укрепление: При <#d94c49ff>падении здоровья ниже ${rphThreshold}%<> даёт <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> как <#60e84dff>дополнительное здоровье<> на <#e8a800ff>${rphDur} секунд<> и <#60e84dff>восстанавливает вам<> половину этого количества (перезарядка ${rphCd} секунд)."
$i18n.ru.terminus.option = "Соприкосновение: При атаке получает либо <#ffdd8eff>${tPen}% <$armorPenIcon> пробивания брони<>, либо <#88ccffff>${tPen}% <$magicPenIcon> пробивания магии<> на <#e8a800ff>${tDur} секунды<>, чередуясь (макс. ${tStacks} стака каждого)."
$i18n.ru.radiant_terminus.option = "Соприкосновение: При атаке получает либо <#ffdd8eff>${rtPen}% <$armorPenIcon> пробивания брони<>, либо <#88ccffff>${rtPen}% <$magicPenIcon> пробивания магии<> на <#e8a800ff>${rtDur} секунды<>, чередуясь (макс. ${rtStacks} стака каждого)."
$i18n.ru.collector.option = "Смерть: Нанесение урона вражеским чемпионам с <#60e84dff>максимальным здоровьем<> ниже <#60e84dff>${colThreshold}%<> <$hpIcon> <#d94c49ff>казнит<> их."
$i18n.ru.radiant_collector.option = "Смерть: Нанесение урона вражеским чемпионам с <#60e84dff>максимальным здоровьем<> ниже <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#d94c49ff>казнит<> их."
$i18n.ru.heartsteel.option = "Железное сердце: Каждые <#e8a800ff>${hsCd} секунд<>, ваша следующая атака наносит <#ff9028ff>дополнительный физический урон<> равный <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>, и даёт <#60e84dff>${hsBonusHpPct}%<> от этого урона как постоянное <#60e84dff>дополнительное здоровье<>."
$i18n.ru.radiant_heartsteel.option = "Железное сердце: Каждые <#e8a800ff>${rhsCd} секунд<>, ваша следующая атака наносит <#ff9028ff>дополнительный физический урон<> равный <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>, и даёт <#60e84dff>${rhsBonusHpPct}%<> от этого урона как постоянное <#60e84dff>дополнительное здоровье<>."
$i18n.ru.spear_of_shojin.option = "Концентрация: Попадания способностями по вражеским чемпионам дают <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>Силы Атаки<> на <#e8a800ff>${sosDur} секунд<> (макс. ${sosStacks} стака)."
$i18n.ru.radiant_spear_of_shojin.option = "Концентрация: Попадания способностями по вражеским чемпионам дают <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>Силы Атаки<> на <#e8a800ff>${rsosDur} секунд<> (макс. ${rsosStacks} стака)."

$i18nJson = $i18n | ConvertTo-Json -Depth 10
$i18nJson = $i18nJson -replace '\\u003c', '<' -replace '\\u003e', '>' -replace '\\u0027', "'"
[System.IO.File]::WriteAllText($i18nPath, $i18nJson)

Write-Host "Done."
Write-Host "  Executioner's Calling:   -${execHeal}% healing / ${execDur}s"
Write-Host "  Oblivion Orb:            -${ooHeal}% healing / ${ooDur}s"
Write-Host "  Morellonomicon:          -${morHeal}% healing / ${morDur}s"
Write-Host "  Radiant Morellonomicon:  -${rmorHeal}% healing / ${rmorDur}s"
Write-Host "  Overlord's Bloodmail:         ${obmAtk}% max HP as AD"
Write-Host "  Radiant Overlord's Bloodmail: ${robmAtk}% max HP as AD"
Write-Host "  Night Harvester:              ${nhFlat} + ${nhApPct}% AP magic dmg / ${nhMs}% MS ${nhDur}s / ${nhCd}s per-target CD"
Write-Host "  Radiant Night Harvester:      ${rnhFlat} + ${rnhApPct}% AP magic dmg / ${rnhMs}% MS ${rnhDur}s / ${rnhCd}s per-target CD"
Write-Host "  Protector's Vow:            ${pvFlat} + ${pvArmorPct}% Armor as HP"
Write-Host "  Radiant Protector's Vow:    ${rpvFlat} + ${rpvArmorPct}% Armor as HP"
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
Write-Host "  Death's Dance:           ${ddDelay}% deferred / ${ddBurnCap}% max HP per second / ${ddHeal}% missing HP heal on kill"
Write-Host "  Radiant Death's Dance:   ${rddDelay}% deferred / ${rddBurnCap}% max HP per second / ${rddHeal}% missing HP heal on kill"
Write-Host "  Rabadon's Deathcap:      ${rabMult}%"
Write-Host "  Radiant Rabadon's:       ${radRabMult}%"
Write-Host "  Mirage Blade:            ${mbForce} force"
Write-Host "  Radiant Mirage Blade:    ${rmbForce} force"
Write-Host "  Spirit Visage:              +${svHeal}% healing"
Write-Host "  Radiant Spirit Visage:      +${rsvHeal}% healing"
Write-Host "  Unending Despair:           ${udFlat} + ${udHpPct}% max HP heal/skill"
Write-Host "  Radiant Unending Despair:   ${rudFlat} + ${rudHpPct}% max HP heal/skill"
Write-Host "  Protoplasm Harness:         ${phThreshold} max HP Threshold / ${phFlat} + ${phHpPct}% max HP / ${phDur}s / ${phCd}s cooldown"
Write-Host "  Radiant Protoplasm Harness: ${rphThreshold} max HP Threshold / ${rphFlat} + ${rphHpPct}% max HP / ${rphDur}s / ${rphCd}s cooldown"
Write-Host "  Terminus:               ${tPen}% pen/stack / ${tDur}s / ${tStacks} stacks"
Write-Host "  Radiant Terminus:       ${rtPen}% pen/stack / ${rtDur}s / ${rtStacks} stacks"
Write-Host "  Collector:              executes below ${colThreshold}% HP"
Write-Host "  Radiant Collector:      executes below ${rcolThreshold}% HP"
Write-Host "  Heartsteel:             ${hsFlat} + ${hsHpPct}% max HP dmg / ${hsBonusHpPct}% bonus HP / ${hsCd}s cooldown"
Write-Host "  Radiant Heartsteel:     ${rhsFlat} + ${rhsHpPct}% max HP dmg / ${rhsBonusHpPct}% bonus HP / ${rhsCd}s cooldown"
Write-Host "  Spear of Shojin:         ${sosAtkMult}% AD/stack / ${sosDur}s / ${sosStacks} stacks"
Write-Host "  Radiant Spear of Shojin: ${rsosAtkMult}% AD/stack / ${rsosDur}s / ${rsosStacks} stacks"
