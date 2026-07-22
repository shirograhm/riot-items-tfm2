$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$configPath = Join-Path $scriptDir "config.json"
$defaultConfigPath = Join-Path $scriptDir "config-default.json"
$i18nPath = Join-Path $scriptDir "text\item.i18n"

# Default values come from config-default.json (one entry per item, mirroring
# the Rust Default impls). config.json, if present, overrides individual fields.
if (-not (Test-Path $defaultConfigPath)) {
    Write-Error "config-default.json not found next to this script."
    exit 1
}
$config = Get-Content $defaultConfigPath -Raw | ConvertFrom-Json

if (Test-Path $configPath) {
    $overrides = Get-Content $configPath -Raw | ConvertFrom-Json
    foreach ($item in $overrides.PSObject.Properties) {
        if (-not $config.PSObject.Properties[$item.Name]) {
            $config | Add-Member -NotePropertyName $item.Name -NotePropertyValue ([PSCustomObject]@{})
        }
        foreach ($field in $item.Value.PSObject.Properties) {
            $config.$($item.Name) | Add-Member -NotePropertyName $field.Name -NotePropertyValue $field.Value -Force
        }
    }
    Write-Host "Applied overrides from config.json."
}
else {
    Write-Host "config.json not found, using config-default.json values."
}

$execHeal = [int]$config.executioners_calling.effect_heal_reduce
$execDur = [int]$config.executioners_calling.effect_duration_seconds
$ooHeal = [int]$config.oblivion_orb.effect_heal_reduce
$ooDur = [int]$config.oblivion_orb.effect_duration_seconds
$obmAtk = [double]$config.overlords_bloodmail.effect_caster_hp_percent_attack
$robmAtk = [double]$config.radiant_overlords_bloodmail.effect_caster_hp_percent_attack
$nhFlat = [int]$config.night_harvester.effect_bonus_flat_damage
$nhApPct = [double]$config.night_harvester.effect_ap_percent_damage
$nhMs = [int]$config.night_harvester.effect_move_speed_mult
$nhDur = [int]$config.night_harvester.effect_duration_seconds
$nhCd = [int]$config.night_harvester.effect_cooldown_seconds
$rnhFlat = [int]$config.radiant_night_harvester.effect_bonus_flat_damage
$rnhApPct = [double]$config.radiant_night_harvester.effect_ap_percent_damage
$rnhMs = [int]$config.radiant_night_harvester.effect_move_speed_mult
$rnhDur = [int]$config.radiant_night_harvester.effect_duration_seconds
$rnhCd = [int]$config.radiant_night_harvester.effect_cooldown_seconds
$pvFlat = [int]$config.protectors_vow.effect_bonus_flat_hp
$pvArmorPct = [double]$config.protectors_vow.effect_caster_defence_percent_hp
$rpvFlat = [int]$config.radiant_protectors_vow.effect_bonus_flat_hp
$rpvArmorPct = [double]$config.radiant_protectors_vow.effect_caster_defence_percent_hp
$ntFlat = [int]$config.nashors_tooth.effect_bonus_flat_damage
$ntApPct = [double]$config.nashors_tooth.effect_ap_percent_damage
$rntFlat = [int]$config.radiant_nashors_tooth.effect_bonus_flat_damage
$rntApPct = [double]$config.radiant_nashors_tooth.effect_ap_percent_damage
$rmHpPct = [double]$config.riftmaker.effect_caster_hp_percent_power
$rmDur = [int]$config.riftmaker.effect_duration_seconds
$rmStacks = [int]$config.riftmaker.effect_max_stacks
$rrmHpPct = [double]$config.radiant_riftmaker.effect_caster_hp_percent_power
$rrmDur = [int]$config.radiant_riftmaker.effect_duration_seconds
$rrmStacks = [int]$config.radiant_riftmaker.effect_max_stacks
$mrHeal = [int]$config.mortal_reminder.effect_heal_reduce
$mrDur = [int]$config.mortal_reminder.effect_duration_seconds
$rmrHeal = [int]$config.radiant_mortal_reminder.effect_heal_reduce
$rmrDur = [int]$config.radiant_mortal_reminder.effect_duration_seconds
$morHeal = [int]$config.morellonomicon.effect_heal_reduce
$morDur = [int]$config.morellonomicon.effect_duration_seconds
$rmorHeal = [int]$config.radiant_morellonomicon.effect_heal_reduce
$rmorDur = [int]$config.radiant_morellonomicon.effect_duration_seconds
$jakDefMult = [int]$config.jaksho_the_protean.effect_stack_defence_mult
$jakMrMult = [int]$config.jaksho_the_protean.effect_stack_magic_resistance_mult
$jakDur = [int]$config.jaksho_the_protean.effect_duration_seconds
$jakStacks = [int]$config.jaksho_the_protean.effect_max_stacks
$rjakDefMult = [int]$config.radiant_jaksho_the_protean.effect_stack_defence_mult
$rjakMrMult = [int]$config.radiant_jaksho_the_protean.effect_stack_magic_resistance_mult
$rjakDur = [int]$config.radiant_jaksho_the_protean.effect_duration_seconds
$rjakStacks = [int]$config.radiant_jaksho_the_protean.effect_max_stacks
$fmSlow = [int]$config.frozen_mallet.effect_slow_amount
$fmDur = [int]$config.frozen_mallet.effect_duration_seconds
$rfmSlow = [int]$config.radiant_frozen_mallet.effect_slow_amount
$rfmDur = [int]$config.radiant_frozen_mallet.effect_duration_seconds
$rfmFlat = [int]$config.radiant_frozen_mallet.effect_bonus_flat_damage
$rfmHpPct = [double]$config.radiant_frozen_mallet.effect_caster_hp_percent_damage
$rcsSlow = [int]$config.rylais_crystal_scepter.effect_slow_amount
$rcsDur = [int]$config.rylais_crystal_scepter.effect_duration_seconds
$rrcsSlow = [int]$config.radiant_rylais_crystal_scepter.effect_slow_amount
$rrcsDur = [int]$config.radiant_rylais_crystal_scepter.effect_duration_seconds
$hexUltCdr = [int]$config.experimental_hexplate.ult_cooldown_mult
$rhexUltCdr = [int]$config.radiant_experimental_hexplate.ult_cooldown_mult
$gbDmg = [int]$config.guinsoos_rageblade.effect_bonus_magic_damage
$gbSpeed = [int]$config.guinsoos_rageblade.effect_stack_attack_speed_mult
$gbDur = [int]$config.guinsoos_rageblade.effect_duration_seconds
$gbStacks = [int]$config.guinsoos_rageblade.effect_max_stacks
$rgbDmg = [int]$config.radiant_guinsoos_rageblade.effect_bonus_magic_damage
$rgbSpeed = [int]$config.radiant_guinsoos_rageblade.effect_stack_attack_speed_mult
$rgbDur = [int]$config.radiant_guinsoos_rageblade.effect_duration_seconds
$rgbStacks = [int]$config.radiant_guinsoos_rageblade.effect_max_stacks
$weDmg = [int]$config.wits_end.effect_bonus_magic_damage
$rweDmg = [int]$config.radiant_wits_end.effect_bonus_magic_damage
$ksDmg = [int]$config.kraken_slayer.effect_bonus_flat_damage
$ksBonus = [int]$config.kraken_slayer.effect_max_percent_bonus
$ksThresh = [int]$config.kraken_slayer.effect_hp_percent_threshold
$rksDmg = [int]$config.radiant_kraken_slayer.effect_bonus_flat_damage
$rksBonus = [int]$config.radiant_kraken_slayer.effect_max_percent_bonus
$rksThresh = [int]$config.radiant_kraken_slayer.effect_hp_percent_threshold
$arCrit = [int]$config.atmas_reckoning.effect_stack_crit_chance
$arHp = [int]$config.atmas_reckoning.effect_hp_per_stack
$arCap = $arCrit * [int]$config.atmas_reckoning.effect_max_stacks
$rarCrit = [int]$config.radiant_atmas_reckoning.effect_stack_crit_chance
$rarHp = [int]$config.radiant_atmas_reckoning.effect_hp_per_stack
$rarCap = $rarCrit * [int]$config.radiant_atmas_reckoning.effect_max_stacks
$ldrPct = [int]$config.lord_dominiks_regards.effect_percent_bonus_damage
$ldrHp = [int]$config.lord_dominiks_regards.effect_hp_per_stack
$ldrMax = [int]$config.lord_dominiks_regards.effect_max_percent_bonus
$rldrPct = [int]$config.radiant_lord_dominiks_regards.effect_percent_bonus_damage
$rldrHp = [int]$config.radiant_lord_dominiks_regards.effect_hp_per_stack
$rldrMax = [int]$config.radiant_lord_dominiks_regards.effect_max_percent_bonus
$bftPower = [int]$config.blackfire_torch.effect_stack_magic_power
$bftDur = [int]$config.blackfire_torch.effect_duration_seconds
$bftStacks = [int]$config.blackfire_torch.effect_max_stacks
$rbftPower = [int]$config.radiant_blackfire_torch.effect_stack_magic_power
$rbftDur = [int]$config.radiant_blackfire_torch.effect_duration_seconds
$rbftStacks = [int]$config.radiant_blackfire_torch.effect_max_stacks
$borkPct = [double]$config.blade_of_the_ruined_king.effect_hp_percent_damage
$borkCap = [int]$config.blade_of_the_ruined_king.effect_minion_damage_cap
$rborkPct = [double]$config.radiant_blade_of_the_ruined_king.effect_hp_percent_damage
$rborkCap = [int]$config.radiant_blade_of_the_ruined_king.effect_minion_damage_cap
$dbMult = [int]$config.deathblade.attack_mult
$rdbMult = [int]$config.radiant_deathblade.attack_mult
$ddDelay = [double]$config.deaths_dance.effect_delayed_damage_percent
$ddBurnCap = [double]$config.deaths_dance.effect_burn_hp_percent_cap
$rddDelay = [double]$config.radiant_deaths_dance.effect_delayed_damage_percent
$rddBurnCap = [double]$config.radiant_deaths_dance.effect_burn_hp_percent_cap
$ddHeal = [double]$config.deaths_dance.effect_kill_heal_missing_percent
$rddHeal = [double]$config.radiant_deaths_dance.effect_kill_heal_missing_percent
$ddFlatHeal = [double]$config.deaths_dance.effect_bonus_flat_heal
$rddFlatHeal = [double]$config.radiant_deaths_dance.effect_bonus_flat_heal
$rabMult = [int]$config.rabadons_deathcap.magic_power_mult
$radRabMult = [int]$config.radiant_rabadons_deathcap.magic_power_mult
$mbForce = [int]$config.mirage_blade.adaptive_force
$rmbForce = [int]$config.radiant_mirage_blade.adaptive_force
$mbMoveSpeed = [int]$config.mirage_blade.effect_move_speed_mult
$rmbMoveSpeed = [int]$config.radiant_mirage_blade.effect_move_speed_mult
$mbDuration = [int]$config.mirage_blade.effect_duration_seconds
$rmbDuration = [int]$config.radiant_mirage_blade.effect_duration_seconds
$dtsForce = [int]$config.diamond_tipped_spear.adaptive_force
$rdtsForce = [int]$config.radiant_diamond_tipped_spear.adaptive_force
$dtsPct = [int]$config.diamond_tipped_spear.effect_max_percent_bonus
$rdtsPct = [int]$config.radiant_diamond_tipped_spear.effect_max_percent_bonus
$dtsDist = [int]$config.diamond_tipped_spear.effect_max_distance
$rdtsDist = [int]$config.radiant_diamond_tipped_spear.effect_max_distance
$zekForce = [int]$config.zekes_herald.effect_adaptive_force
$rzekForce = [int]$config.radiant_zekes_herald.effect_adaptive_force
$zekDist = [int]$config.zekes_herald.effect_max_distance
$rzekDist = [int]$config.radiant_zekes_herald.effect_max_distance
$zekVamp = [int]$config.zekes_herald.effect_vamp
$rzekVamp = [int]$config.radiant_zekes_herald.effect_vamp
$ssDmg = [int]$config.scouts_slingshot.effect_bonus_flat_damage
$ssCd = [int]$config.scouts_slingshot.effect_cooldown_seconds
$litHp = [int]$config.liandrys_torment.effect_hp_percent_damage
$rlitHp = [int]$config.radiant_liandrys_torment.effect_hp_percent_damage
$litDur = [int]$config.liandrys_torment.effect_duration_seconds
$rlitDur = [int]$config.radiant_liandrys_torment.effect_duration_seconds
$litCap = [int]$config.liandrys_torment.effect_minion_damage_cap
$rlitCap = [int]$config.radiant_liandrys_torment.effect_minion_damage_cap
$svHeal = [double]$config.spirit_visage.effect_heal_mult
$rsvHeal = [double]$config.radiant_spirit_visage.effect_heal_mult
$udFlat = [int]$config.unending_despair.effect_bonus_flat_heal
$udHpPct = [double]$config.unending_despair.effect_caster_hp_percent_heal
$rudFlat = [int]$config.radiant_unending_despair.effect_bonus_flat_heal
$rudHpPct = [double]$config.radiant_unending_despair.effect_caster_hp_percent_heal
$phThreshold = [int]$config.protoplasm_harness.effect_hp_percent_threshold
$phFlat = [int]$config.protoplasm_harness.effect_bonus_flat_hp
$phHpPct = [double]$config.protoplasm_harness.effect_hp_percent_boost
$phDur = [int]$config.protoplasm_harness.effect_duration_seconds
$phCd = [int]$config.protoplasm_harness.effect_cooldown_seconds
$rphThreshold = [int]$config.radiant_protoplasm_harness.effect_hp_percent_threshold
$rphFlat = [int]$config.radiant_protoplasm_harness.effect_bonus_flat_hp
$rphHpPct = [double]$config.radiant_protoplasm_harness.effect_hp_percent_boost
$rphDur = [int]$config.radiant_protoplasm_harness.effect_duration_seconds
$rphCd = [int]$config.radiant_protoplasm_harness.effect_cooldown_seconds
$tArmorPen = [int]$config.terminus.effect_armor_pen_per_stack
$tMagicPen = [int]$config.terminus.effect_magic_pen_per_stack
$tDur = [int]$config.terminus.effect_duration_seconds
$tStacks = [int]$config.terminus.effect_max_stacks
$rtArmorPen = [int]$config.radiant_terminus.effect_armor_pen_per_stack
$rtMagicPen = [int]$config.radiant_terminus.effect_magic_pen_per_stack
$rtDur = [int]$config.radiant_terminus.effect_duration_seconds
$rtStacks = [int]$config.radiant_terminus.effect_max_stacks
$colThreshold = [double]$config.collector.effect_hp_percent_threshold
$rcolThreshold = [double]$config.radiant_collector.effect_hp_percent_threshold
$sfThreshold = [double]$config.shadowflame.effect_hp_percent_threshold
$rsfThreshold = [double]$config.radiant_shadowflame.effect_hp_percent_threshold
$hsFlat = [int]$config.heartsteel.effect_bonus_flat_damage
$hsHpPct = [double]$config.heartsteel.effect_caster_hp_percent_damage
$hsBonusHpPct = [double]$config.heartsteel.effect_bonus_hp_percent_of_damage
$hsCd = [int]$config.heartsteel.effect_cooldown_seconds
$rhsFlat = [int]$config.radiant_heartsteel.effect_bonus_flat_damage
$rhsHpPct = [double]$config.radiant_heartsteel.effect_caster_hp_percent_damage
$rhsBonusHpPct = [double]$config.radiant_heartsteel.effect_bonus_hp_percent_of_damage
$rhsCd = [int]$config.radiant_heartsteel.effect_cooldown_seconds
$sosAtkMult = [int]$config.spear_of_shojin.effect_stack_attack_mult
$sosDur = [int]$config.spear_of_shojin.effect_duration_seconds
$sosStacks = [int]$config.spear_of_shojin.effect_max_stacks
$rsosAtkMult = [int]$config.radiant_spear_of_shojin.effect_stack_attack_mult
$rsosDur = [int]$config.radiant_spear_of_shojin.effect_duration_seconds
$rsosStacks = [int]$config.radiant_spear_of_shojin.effect_max_stacks
$waHeal = [double]$config.warmogs_armor.effect_caster_hp_percent_heal
$waMs = [int]$config.warmogs_armor.effect_move_speed_mult
$waDur = [int]$config.warmogs_armor.effect_duration_seconds
$rwaHeal = [double]$config.radiant_warmogs_armor.effect_caster_hp_percent_heal
$rwaMs = [int]$config.radiant_warmogs_armor.effect_move_speed_mult
$rwaDur = [int]$config.radiant_warmogs_armor.effect_duration_seconds
$ytCrit = [int]$config.yun_tal_wildarrows.effect_stack_crit_chance
$ytStacks = [int]$config.yun_tal_wildarrows.effect_max_stacks
$ytFlurryAS = [int]$config.yun_tal_wildarrows.effect_flurry_attack_speed_mult
$ytDur = [int]$config.yun_tal_wildarrows.effect_duration_seconds
$ytCd = [int]$config.yun_tal_wildarrows.effect_cooldown_seconds
$ytMaxCrit = $ytCrit * $ytStacks
$rytCrit = [int]$config.radiant_yun_tal_wildarrows.effect_stack_crit_chance
$rytStacks = [int]$config.radiant_yun_tal_wildarrows.effect_max_stacks
$rytFlurryAS = [int]$config.radiant_yun_tal_wildarrows.effect_flurry_attack_speed_mult
$rytDur = [int]$config.radiant_yun_tal_wildarrows.effect_duration_seconds
$rytCd = [int]$config.radiant_yun_tal_wildarrows.effect_cooldown_seconds
$rytMaxCrit = $rytCrit * $rytStacks
$srStacks = [int]$config.stormrazor.effect_max_stacks
$srMs = [int]$config.stormrazor.effect_move_speed_mult
$srDmg = [int]$config.stormrazor.effect_bonus_flat_damage
$srDur = [double]$config.stormrazor.effect_duration_seconds
$rsrStacks = [int]$config.radiant_stormrazor.effect_max_stacks
$rsrMs = [int]$config.radiant_stormrazor.effect_move_speed_mult
$rsrDmg = [int]$config.radiant_stormrazor.effect_bonus_flat_damage
$rsrDur = [double]$config.radiant_stormrazor.effect_duration_seconds
$bcShred = [int]$config.black_cleaver.effect_percent_armor_shred
$bcDur = [int]$config.black_cleaver.effect_duration_seconds
$bcStacks = [int]$config.black_cleaver.effect_max_stacks
$rbcShred = [int]$config.radiant_black_cleaver.effect_percent_armor_shred
$rbcDur = [int]$config.radiant_black_cleaver.effect_duration_seconds
$rbcStacks = [int]$config.radiant_black_cleaver.effect_max_stacks
$blcShred = [int]$config.bloodletters_curse.effect_percent_mr_shred
$blcDur = [int]$config.bloodletters_curse.effect_duration_seconds
$blcStacks = [int]$config.bloodletters_curse.effect_max_stacks
$rblcShred = [int]$config.radiant_bloodletters_curse.effect_percent_mr_shred
$rblcDur = [int]$config.radiant_bloodletters_curse.effect_duration_seconds
$rblcStacks = [int]$config.radiant_bloodletters_curse.effect_max_stacks
$ssDamage = [double]$config.sundered_sky.effect_percent_bonus_damage
$ssFlatHeal = [double]$config.sundered_sky.effect_bonus_flat_heal
$ssPercentHeal = [double]$config.sundered_sky.effect_caster_hp_percent_heal
$ssOnHitCD = [double]$config.sundered_sky.on_hit_cooldown_seconds
$rssDamage = [double]$config.radiant_sundered_sky.effect_percent_bonus_damage
$rssFlatHeal = [double]$config.radiant_sundered_sky.effect_bonus_flat_heal
$rssPercentHeal = [double]$config.radiant_sundered_sky.effect_caster_hp_percent_heal
$rssOnHitCD = [double]$config.radiant_sundered_sky.on_hit_cooldown_seconds
$eohConversion = [int]$config.echoes_of_helia.effect_damage_conversion
$eohMinCap = [int]$config.echoes_of_helia.effect_min_stacks
$eohMaxCap = [int]$config.echoes_of_helia.effect_max_stacks
$reohConversion = [int]$config.radiant_echoes_of_helia.effect_damage_conversion
$reohMinCap = [int]$config.radiant_echoes_of_helia.effect_min_stacks
$reohMaxCap = [int]$config.radiant_echoes_of_helia.effect_max_stacks
$sheenMin = [int]$config.sheen.effect_min_bonus_damage
$sheenMax = [int]$config.sheen.effect_max_bonus_damage
$sheenCd = [double]$config.sheen.effect_cooldown_seconds
$tfFlat = [int]$config.trinity_force.effect_bonus_flat_damage
$tfAdPct = [double]$config.trinity_force.effect_ad_percent_damage
$tfCd = [double]$config.trinity_force.effect_cooldown_seconds
$rtfFlat = [int]$config.radiant_trinity_force.effect_bonus_flat_damage
$rtfAdPct = [double]$config.radiant_trinity_force.effect_ad_percent_damage
$rtfCd = [double]$config.radiant_trinity_force.effect_cooldown_seconds
$dndFlat = [int]$config.dusk_and_dawn.effect_bonus_flat_damage
$dndApPct = [double]$config.dusk_and_dawn.effect_ap_percent_damage
$dndApHeal = [double]$config.dusk_and_dawn.effect_caster_ap_percent_heal
$dndHpHeal = [double]$config.dusk_and_dawn.effect_caster_hp_percent_heal
$dndCd = [double]$config.dusk_and_dawn.effect_cooldown_seconds
$rdndFlat = [int]$config.radiant_dusk_and_dawn.effect_bonus_flat_damage
$rdndApPct = [double]$config.radiant_dusk_and_dawn.effect_ap_percent_damage
$rdndApHeal = [double]$config.radiant_dusk_and_dawn.effect_caster_ap_percent_heal
$rdndHpHeal = [double]$config.radiant_dusk_and_dawn.effect_caster_hp_percent_heal
$rdndCd = [double]$config.radiant_dusk_and_dawn.effect_cooldown_seconds
$bsMin = [int]$config.bloodsong.effect_min_bonus_damage
$bsMax = [int]$config.bloodsong.effect_max_bonus_damage
$bsCd = [double]$config.bloodsong.effect_cooldown_seconds
$bsAmp = [int]$config.bloodsong.effect_damaged_amplify
$bsDur = [int]$config.bloodsong.effect_duration_seconds
$rbsMin = [int]$config.radiant_bloodsong.effect_min_bonus_damage
$rbsMax = [int]$config.radiant_bloodsong.effect_max_bonus_damage
$rbsCd = [double]$config.radiant_bloodsong.effect_cooldown_seconds
$rbsAmp = [int]$config.radiant_bloodsong.effect_damaged_amplify
$rbsDur = [int]$config.radiant_bloodsong.effect_duration_seconds
$sdLeth = [int]$config.serrated_dirk.effect_lethality
$hubLeth = [int]$config.hubris.effect_lethality
$hubBase = [int]$config.hubris.effect_bonus_flat_attack
$hubStack = [int]$config.hubris.effect_stack_attack
$hubDur = [int]$config.hubris.effect_duration_seconds
$rhubLeth = [int]$config.radiant_hubris.effect_lethality
$rhubBase = [int]$config.radiant_hubris.effect_bonus_flat_attack
$rhubStack = [int]$config.radiant_hubris.effect_stack_attack
$rhubDur = [int]$config.radiant_hubris.effect_duration_seconds
$bbLeth = [int]$config.bastionbreaker.effect_lethality
$bbFlat = [int]$config.bastionbreaker.effect_bonus_flat_damage
$bbPct = [int]$config.bastionbreaker.effect_ad_percent_damage
$bbDur = [int]$config.bastionbreaker.effect_duration_seconds
$rbbLeth = [int]$config.radiant_bastionbreaker.effect_lethality
$rbbFlat = [int]$config.radiant_bastionbreaker.effect_bonus_flat_damage
$rbbPct = [int]$config.radiant_bastionbreaker.effect_ad_percent_damage
$rbbDur = [int]$config.radiant_bastionbreaker.effect_duration_seconds
$spfLeth = [int]$config.serpents_fang.effect_lethality
$spfFlat = [int]$config.serpents_fang.effect_bonus_flat_damage
$spfPct = [int]$config.serpents_fang.effect_ad_percent_damage
$rspfLeth = [int]$config.radiant_serpents_fang.effect_lethality
$rspfFlat = [int]$config.radiant_serpents_fang.effect_bonus_flat_damage
$rspfPct = [int]$config.radiant_serpents_fang.effect_ad_percent_damage

$i18n = Get-Content $i18nPath -Raw -Encoding UTF8 | ConvertFrom-Json

$adIcon = "i#asset/base/ui/banpick/champion_stat_icon:ad_0"
$armorIcon = "i#asset/base/ui/banpick/champion_stat_icon:armor_0"
$hpIcon = "i#asset/base/ui/banpick/champion_stat_icon:hp_0"
$mrIcon = "i#asset/base/ui/banpick/champion_stat_icon:magic resistance_0"
$apIcon = "i#asset/base/ui/banpick/champion_stat_icon:ap_0"
$asIcon = "i#asset/base/ui/banpick/champion_stat_icon:attack_speed_0"
$forceIcon = "i#asset/base/ui/banpick/champion_stat_icon:force_0"
$speedIcon = "i#asset/base/ui/banpick/champion_stat_icon:speed_0"
$rangeIcon = "i#asset/base/ui/banpick/champion_stat_icon:range_0"
$cdrIcon = "i#asset/base/ui/banpick/champion_stat_icon:cdr_0"
$armorPenIcon = "i#asset/base/ui/banpick/champion_stat_icon:armor_pen_0"
$magicPenIcon = "i#asset/base/ui/banpick/champion_stat_icon:magic_pen_0"
$critIcon = "i#asset/base/ui/banpick/champion_stat_icon:crit_chance_0"
$vampIcon = "i#asset/base/ui/banpick/champion_stat_icon:vamp_0"
$levelIcon = "i#asset/base/ui/banpick/champion_stat_icon:level_0"

Write-Host "Updating English text."

$i18n.en.executioners_calling.option = "Grievous Wounds: Dealing <#ff9028ff>physical damage<> to an enemy champion <#d94c49ff>reduces their healing by ${execHeal}%<> for <#e8a800ff>${execDur} seconds<>."
$i18n.en.oblivion_orb.option = "Grievous Wounds: Dealing <#a974ffff>magic damage<> to an enemy champion <#d94c49ff>reduces their healing by ${ooHeal}%<> for <#e8a800ff>${ooDur} seconds<>."
$i18n.en.morellonomicon.option = "Grievous Wounds: Dealing <#a974ffff>magic damage<> to an enemy champion <#d94c49ff>reduces their healing by ${morHeal}%<> for <#e8a800ff>${morDur} seconds<>."
$i18n.en.radiant_morellonomicon.option = "Grievous Wounds: Dealing <#a974ffff>magic damage<> to an enemy champion <#d94c49ff>reduces their healing by ${rmorHeal}%<> for <#e8a800ff>${rmorDur} seconds<>."
$i18n.en.overlords_bloodmail.option = "Tyranny: Gain <#ff9028ff>bonus<> <$adIcon> <#ff9028ff>Attack Damage<> equal to <#60e84dff>${obmAtk}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.radiant_overlords_bloodmail.option = "Tyranny: Gain <#ff9028ff>bonus<> <$adIcon> <#ff9028ff>Attack Damage<> equal to <#60e84dff>${robmAtk}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.night_harvester.option = "Soulrend: Landing an Ability on an enemy champion deals <#a974ffff>bonus magic damage<> equal to <#a974ffff>${nhFlat}<> + <#a974ffff>${nhApPct}%<> <$apIcon> <#a974ffff>Ability Power<> and grants <#ffffffff>${nhMs}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>${nhDur} seconds<> (<#e8a800ff>${nhCd} second<> cooldown per target)."
$i18n.en.radiant_night_harvester.option = "Soulrend: Landing an Ability on an enemy champion deals <#a974ffff>bonus magic damage<> equal to <#a974ffff>${rnhFlat}<> + <#a974ffff>${rnhApPct}%<> <$apIcon> <#a974ffff>Ability Power<> and grants <#ffffffff>${rnhMs}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>${rnhDur} seconds<> (<#e8a800ff>${rnhCd} second<> cooldown per target)."
$i18n.en.protectors_vow.option = "Awe: Gain <#60e84dff>bonus health<> equal to <#60e84dff>${pvFlat}<> + <#ffdd8eff>${pvArmorPct}%<> of your <$armorIcon> <#ffdd8eff>Armor<>."
$i18n.en.radiant_protectors_vow.option = "Awe: Gain <#60e84dff>bonus health<> equal to <#60e84dff>${rpvFlat}<> + <#ffdd8eff>${rpvArmorPct}%<> of your <$armorIcon> <#ffdd8eff>Armor<>."
$i18n.en.nashors_tooth.option = "Icathian Bite: On attack, deal <#a974ffff>bonus magic damage<> equal to <#a974ffff>${ntFlat}<> + <#a974ffff>${ntApPct}%<> <$apIcon> <#a974ffff>Ability Power<>."
$i18n.en.radiant_nashors_tooth.option = "Icathian Bite: On attack, deal <#a974ffff>bonus magic damage<> equal to <#a974ffff>${rntFlat}<> + <#a974ffff>${rntApPct}%<> <$apIcon> <#a974ffff>Ability Power<>."
$i18n.en.riftmaker.option = "Infusion: Landing an Ability on an enemy champion grants <$apIcon> <#a974ffff>Ability Power<> equal to <#60e84dff>${rmHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> for <#e8a800ff>${rmDur} seconds<> (max ${rmStacks} stacks)."
$i18n.en.radiant_riftmaker.option = "Infusion: Landing an Ability on an enemy champion grants <$apIcon> <#a974ffff>Ability Power<> equal to <#60e84dff>${rrmHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> for <#e8a800ff>${rrmDur} seconds<> (max ${rrmStacks} stacks)."
$i18n.en.shadowflame.option = "Cinderbloom: Your <#a974ffff>magic damage<> is <#e8a800ff>20% stronger<> against enemies <#d94c49ff>below ${sfThreshold}% maximum health<>."
$i18n.en.radiant_shadowflame.option = "Cinderbloom: Your <#a974ffff>magic damage<> is <#e8a800ff>20% stronger<> against enemies <#d94c49ff>below ${rsfThreshold}% maximum health<>."
$i18n.en.yun_tal_wildarrows.option = "Practice Makes Lethal: Dealing <#ff9028ff>physical damage<> grants <#d45656ff>${ytCrit}%<> <$critIcon> <#d45656ff>critical strike chance<> permanently, up to <#d45656ff>${ytMaxCrit}%<> <$critIcon> .`n`nFlurry: On attack, gain <#ceff99ff>${ytFlurryAS}%<> <$asIcon> <#ceff99ff>attack speed<> for <#e8a800ff>${ytDur} seconds<> (<#e8a800ff>${ytCd} second<> cooldown)."
$i18n.en.radiant_yun_tal_wildarrows.option = "Practice Makes Lethal: Dealing <#ff9028ff>physical damage<> grants <#d45656ff>${rytCrit}%<> <$critIcon> <#d45656ff>critical strike chance<> permanently, up to <#d45656ff>${rytMaxCrit}%<> <$critIcon> .`n`nFlurry: On attack, gain <#ceff99ff>${rytFlurryAS}%<> <$asIcon> <#ceff99ff>attack speed<> for <#e8a800ff>${rytDur} seconds<> (<#e8a800ff>${rytCd} second<> cooldown)."
$i18n.en.mortal_reminder.option = "Grievous Wounds: Dealing <#ff9028ff>physical damage<> to an enemy champion <#d94c49ff>reduces their healing by ${mrHeal}%<> for <#e8a800ff>${mrDur} seconds<>."
$i18n.en.radiant_mortal_reminder.option = "Grievous Wounds: Dealing <#ff9028ff>physical damage<> to an enemy champion <#d94c49ff>reduces their healing by ${rmrHeal}%<> for <#e8a800ff>${rmrDur} seconds<>."
$i18n.en.jaksho_the_protean.option = "Resilience: Taking damage from an enemy champion grants <#ffdd8eff>${jakDefMult}% <$armorIcon> armor<> and <#88ccffff>${jakMrMult}% <$mrIcon> magic resistance<> for <#e8a800ff>${jakDur} seconds<> (max ${jakStacks} stacks)."
$i18n.en.radiant_jaksho_the_protean.option = "Resilience: Taking damage from an enemy champion grants <#ffdd8eff>${rjakDefMult}% <$armorIcon> armor<> and <#88ccffff>${rjakMrMult}% <$mrIcon> magic resistance<> for <#e8a800ff>${rjakDur} seconds<> (max ${rjakStacks} stacks)."
$i18n.en.frozen_mallet.option = "Icy: On attack, apply a <#d94c49ff>${fmSlow}% slow<> for <#e8a800ff>${fmDur} seconds<>."
$i18n.en.radiant_frozen_mallet.option = "Icy: On attack, deal <#ff9028ff>bonus physical damage<> equal to <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> and apply a <#d94c49ff>${rfmSlow}% slow<> for <#e8a800ff>${rfmDur} seconds<>."
$i18n.en.rylais_crystal_scepter.option = "Rimefrost: Dealing Ability damage applies a <#d94c49ff>${rcsSlow}% slow<> for <#e8a800ff>${rcsDur} seconds<>."
$i18n.en.radiant_rylais_crystal_scepter.option = "Rimefrost: Dealing Ability damage applies a <#d94c49ff>${rrcsSlow}% slow<> for <#e8a800ff>${rrcsDur} seconds<>."
$i18n.en.experimental_hexplate.option = "Overdrive: Gain <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>cooldown reduction<> on your ultimate skill."
$i18n.en.radiant_experimental_hexplate.option = "Overdrive: Gain <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>cooldown reduction<> on your ultimate skill."
$i18n.en.guinsoos_rageblade.option = "Wrath: Your basic attacks deal <#a974ffff>${gbDmg} bonus magic damage<>.`n`nSeething Strike: On attack, gain <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>attack speed<> for <#e8a800ff>${gbDur} seconds<> (max ${gbStacks} stacks)."
$i18n.en.radiant_guinsoos_rageblade.option = "Wrath: Your basic attacks deal <#a974ffff>${rgbDmg} bonus magic damage<>.`n`nSeething Strike: On attack, gain <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>attack speed<> for <#e8a800ff>${rgbDur} seconds<> (max ${rgbStacks} stacks)."
$i18n.en.wits_end.option = "Fray: Your basic attacks deal <#a974ffff>${weDmg} bonus magic damage<>."
$i18n.en.radiant_wits_end.option = "Fray: Your basic attacks deal <#a974ffff>${rweDmg} bonus magic damage<>."
$i18n.en.kraken_slayer.option = "Bring It Down: Every third basic attack deals <#ff9028ff>${ksDmg} bonus physical damage<>, increased by up to <#ff9028ff>${ksBonus}%<> based on the target's <#60e84dff>missing health<> (maximum bonus at <#60e84dff>${ksThresh}%<> target health)."
$i18n.en.radiant_kraken_slayer.option = "Bring It Down: Every third basic attack deals <#ff9028ff>${rksDmg} bonus physical damage<>, increased by up to <#ff9028ff>${rksBonus}%<> based on the target's <#60e84dff>missing health<> (maximum bonus at <#60e84dff>${rksThresh}%<> target health)."
$i18n.en.atmas_reckoning.option = "Big Hands: Gain <#d45656ff>${arCrit}%<> <$critIcon> <#d45656ff>critical strike chance<> for every <#60e84dff>${arHp}<> <$hpIcon> <#60e84dff>maximum health<>, up to <#d45656ff>${arCap}%<> <$critIcon> ."
$i18n.en.radiant_atmas_reckoning.option = "Big Hands: Gain <#d45656ff>${rarCrit}%<> <$critIcon> <#d45656ff>critical strike chance<> for every <#60e84dff>${rarHp}<> <$hpIcon> <#60e84dff>maximum health<>, up to <#d45656ff>${rarCap}%<> <$critIcon> ."
$i18n.en.lord_dominiks_regards.option = "Giant Slayer: Deal <#ff9028ff>${ldrPct}% bonus damage<> for every <#60e84dff>${ldrHp}<> <$hpIcon> <#60e84dff>maximum health<> the target has, up to <#ff9028ff>${ldrMax}%<>."
$i18n.en.radiant_lord_dominiks_regards.option = "Giant Slayer: Deal <#ff9028ff>${rldrPct}% bonus damage<> for every <#60e84dff>${rldrHp}<> <$hpIcon> <#60e84dff>maximum health<> the target has, up to <#ff9028ff>${rldrMax}%<>."
$i18n.en.blackfire_torch.option = "Maleficent: Landing an Ability on an enemy champion grants <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>Ability Power<> for <#e8a800ff>${bftDur} seconds<> (max ${bftStacks} stacks)."
$i18n.en.radiant_blackfire_torch.option = "Maleficent: Landing an Ability on an enemy champion grants <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>Ability Power<> for <#e8a800ff>${rbftDur} seconds<> (max ${rbftStacks} stacks)."
$i18n.en.blade_of_the_ruined_king.option = "Mist's Edge: On attack, deal <#ff9028ff>bonus physical damage<> equal to <#d94c49ff>${borkPct}% of the target's current health<>. Deals a maximum of <#ff9028ff>${borkCap} physical damage<> against minions and monsters."
$i18n.en.radiant_blade_of_the_ruined_king.option = "Mist's Edge: On attack, deal <#ff9028ff>bonus physical damage<> equal to <#d94c49ff>${rborkPct}% of the target's current health<>. Deals a maximum of <#ff9028ff>${rborkCap} physical damage<> against minions and monsters."
$i18n.en.deathblade.option = "Apex: Increase your total <$adIcon> <#ff9028ff>Attack Damage<> by <#ff9028ff>${dbMult}%<>."
$i18n.en.radiant_deathblade.option = "Apex: Increase your total <$adIcon> <#ff9028ff>Attack Damage<> by <#ff9028ff>${rdbMult}%<>."
$i18n.en.deaths_dance.option = "Ignore Pain: <#e8a800ff>${ddDelay}%<> of the damage you take is dealt over time as <#ff9028ff>physical damage<> (up to <#60e84dff>${ddBurnCap}%<> of your <$hpIcon> <#60e84dff>maximum health<> per second).`n`nDefy: Scoring a <#e8a800ff>takedown<> on an enemy champion cleanses the remaining stored damage and <#60e84dff>heals you<> for <#60e84dff>${ddFlatHeal}<> + <#60e84dff>${ddHeal}%<> of your <#60e84dff>missing health<>."
$i18n.en.radiant_deaths_dance.option = "Ignore Pain: <#e8a800ff>${rddDelay}%<> of the damage you take is dealt over time as <#ff9028ff>physical damage<> (up to <#60e84dff>${rddBurnCap}%<> of your <$hpIcon> <#60e84dff>maximum health<> per second).`n`nDefy: Scoring a <#e8a800ff>takedown<> on an enemy champion cleanses the remaining stored damage and <#60e84dff>heals you<> for <#60e84dff>${rddFlatHeal}<> + <#60e84dff>${rddHeal}%<> of your <#60e84dff>missing health<>."
$i18n.en.rabadons_deathcap.option = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${rabMult}%<>."
$i18n.en.radiant_rabadons_deathcap.option = "Opus: Increase your total <$apIcon> <#a974ffff>Ability Power<> by <#a974ffff>${radRabMult}%<>."

$mbIllusion = "Illusion: Gain <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Adaptive Force<>. Each <$forceIcon> <#d48294ff>Adaptive Force<> grants <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Attack Damage<> or <#a974ffff>1<> <$apIcon> <#a974ffff>Ability Power<>, depending on which is higher.`n`nBlur: On kill, gain <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>{2} seconds<>."
$i18n.en.mirage_blade.option = $mbIllusion -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.en.radiant_mirage_blade.option = $mbIllusion -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$dtsPierce = "Pierce: Gain <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Adaptive Force<>. Each <$forceIcon> <#d48294ff>Adaptive Force<> grants <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Attack Damage<> or <#a974ffff>1<> <$apIcon> <#a974ffff>Ability Power<>, depending on which is higher.`n`nSweet Spot: Deal up to <#e8a800ff>{1}% bonus damage<> to enemy champions based on distance (maximum effect at <#ff86c2ff>{2} <$rangeIcon> range<>)."
$i18n.en.diamond_tipped_spear.option = $dtsPierce -f $dtsForce, $dtsPct, $dtsDist
$i18n.en.radiant_diamond_tipped_spear.option = $dtsPierce -f $rdtsForce, $rdtsPct, $rdtsDist
$zekAura = "Aura: Grant <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Adaptive Force<> and <#b7462dff>{2}%<> <$vampIcon> <#b7462dff>Omnivamp<> to all allied champions within <#ff86c2ff>{1} <$rangeIcon> range<>."
$i18n.en.zekes_herald.option = $zekAura -f $zekForce, $zekDist, $zekVamp
$i18n.en.radiant_zekes_herald.option = $zekAura -f $rzekForce, $rzekDist, $rzekVamp
$ssBullseye = "Bullseye: Damaging an enemy champion deals <#a974ffff>{0} bonus magic damage<> (<#e8a800ff>{1} second<> cooldown)."
$i18n.en.scouts_slingshot.option = $ssBullseye -f $ssDmg, $ssCd
$litSuffering = "Suffering: Dealing Ability damage burns enemies, causing them to take <#d94c49ff>{0}% of their maximum health<> as <#a974ffff>magic damage<> over <#e8a800ff>{1} seconds<>. Deals a maximum of <#a974ffff>{2} magic damage<> per tick against minions and monsters."
$i18n.en.liandrys_torment.option = $litSuffering -f $litHp, $litDur, $litCap
$i18n.en.radiant_liandrys_torment.option = $litSuffering -f $rlitHp, $rlitDur, $rlitCap
$i18n.en.spirit_visage.option = "Vitality: Increase all <#60e84dff>healing received<> by <#60e84dff>${svHeal}%<>."
$i18n.en.radiant_spirit_visage.option = "Vitality: Increase all <#60e84dff>healing received<> by <#60e84dff>${rsvHeal}%<>."
$i18n.en.unending_despair.option = "Anguish: Landing an Ability on an enemy champion heals you for <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.radiant_unending_despair.option = "Anguish: Landing an Ability on an enemy champion heals you for <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>."
$i18n.en.protoplasm_harness.option = "Fortification: <#d94c49ff>Falling below ${phThreshold}% health<> grants <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> as <#60e84dff>bonus health<> for <#e8a800ff>${phDur} seconds<> and <#60e84dff>heals you<> for half that amount (<#e8a800ff>${phCd} second<> cooldown)."
$i18n.en.radiant_protoplasm_harness.option = "Fortification: <#d94c49ff>Falling below ${rphThreshold}% health<> grants <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<> as <#60e84dff>bonus health<> for <#e8a800ff>${rphDur} seconds<> and <#60e84dff>heals you<> for half that amount (<#e8a800ff>${rphCd} second<> cooldown)."
$i18n.en.terminus.option = "Juxtaposition: On attack, gain either <#ffdd8eff>${tArmorPen}% <$armorPenIcon> armor penetration<> or <#88ccffff>${tMagicPen}% <$magicPenIcon> magic resistance penetration<> for <#e8a800ff>${tDur} seconds<>, alternating (max ${tStacks} stacks each)."
$i18n.en.radiant_terminus.option = "Juxtaposition: On attack, gain either <#ffdd8eff>${rtArmorPen}% <$armorPenIcon> armor penetration<> or <#88ccffff>${rtMagicPen}% <$magicPenIcon> magic resistance penetration<> for <#e8a800ff>${rtDur} seconds<>, alternating (max ${rtStacks} stacks each)."
$i18n.en.collector.option = "Death: Dealing damage to enemy champions below <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>maximum health<> <#d94c49ff>executes<> them."
$i18n.en.radiant_collector.option = "Death: Dealing damage to enemy champions below <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#60e84dff>maximum health<> <#d94c49ff>executes<> them."
$i18n.en.heartsteel.option = "Ironheart: Every <#e8a800ff>${hsCd} seconds<>, your next attack deals <#ff9028ff>bonus physical damage<> equal to <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>, granting <#60e84dff>${hsBonusHpPct}%<> of that damage as permanent <#60e84dff>bonus health<>."
$i18n.en.radiant_heartsteel.option = "Ironheart: Every <#e8a800ff>${rhsCd} seconds<>, your next attack deals <#ff9028ff>bonus physical damage<> equal to <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> of your <$hpIcon> <#60e84dff>maximum health<>, granting <#60e84dff>${rhsBonusHpPct}%<> of that damage as permanent <#60e84dff>bonus health<>."
$i18n.en.spear_of_shojin.option = "Focused Will: Landing an Ability on an enemy champion grants <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>Attack Damage<> for <#e8a800ff>${sosDur} seconds<> (max ${sosStacks} stacks)."
$i18n.en.radiant_spear_of_shojin.option = "Focused Will: Landing an Ability on an enemy champion grants <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>Attack Damage<> for <#e8a800ff>${rsosDur} seconds<> (max ${rsosStacks} stacks)."
$i18n.en.warmogs_armor.option = "Warmog's Heart: Regenerate <#60e84dff>${waHeal}%<> of your <$hpIcon> <#60e84dff>maximum health<> every second and gain <#ffffffff>${waMs}%<> <$speedIcon> <#ffffffff>movement speed<> if you have not taken damage in the last <#e8a800ff>${waDur} seconds<>."
$i18n.en.radiant_warmogs_armor.option = "Warmog's Heart: Regenerate <#60e84dff>${rwaHeal}%<> of your <$hpIcon> <#60e84dff>maximum health<> every second and gain <#ffffffff>${rwaMs}%<> <$speedIcon> <#ffffffff>movement speed<> if you have not taken damage in the last <#e8a800ff>${rwaDur} seconds<>."
$i18n.en.stormrazor.option = "Energized: Moving and dealing <#ff9028ff>physical damage<> generates <#e8a800ff>Energize<> stacks, up to <#e8a800ff>${srStacks}<>.`n`nBolt: When fully <#e8a800ff>Energized<>, your next instance of <#ff9028ff>physical damage<> deals <#a974ffff>${srDmg} bonus magic damage<> and grants you <#ffffffff>${srMs}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>${srDur} seconds<>."
$i18n.en.radiant_stormrazor.option = "Energized: Moving and dealing <#ff9028ff>physical damage<> generates <#e8a800ff>Energize<> stacks, up to <#e8a800ff>${rsrStacks}<>.`n`nBolt: When fully <#e8a800ff>Energized<>, your next instance of <#ff9028ff>physical damage<> deals <#a974ffff>${rsrDmg} bonus magic damage<> and grants you <#ffffffff>${rsrMs}%<> <$speedIcon> <#ffffffff>movement speed<> for <#e8a800ff>${rsrDur} seconds<>."

$i18n.en.black_cleaver.option = "Carve: Dealing <#ff9028ff>physical damage<> to enemy champions <#d94c49ff>reduces their <$armorIcon> <#ffdd8eff>Armor<> by ${bcShred}%<> for <#e8a800ff>${bcDur} seconds<> (max ${bcStacks} stacks)."
$i18n.en.radiant_black_cleaver.option = "Carve: Dealing <#ff9028ff>physical damage<> to enemy champions <#d94c49ff>reduces their <$armorIcon> <#ffdd8eff>Armor<> by ${rbcShred}%<> for <#e8a800ff>${rbcDur} seconds<> (max ${rbcStacks} stacks)."
$i18n.en.bloodletters_curse.option = "Decay: Dealing <#a974ffff>magic damage<> to enemy champions <#d94c49ff>reduces their <$mrIcon> <#88ccffff>magic resistance<> by ${blcShred}%<> for <#e8a800ff>${blcDur} seconds<> (max ${blcStacks} stacks)."
$i18n.en.radiant_bloodletters_curse.option = "Decay: Dealing <#a974ffff>magic damage<> to enemy champions <#d94c49ff>reduces their <$mrIcon> <#88ccffff>magic resistance<> by ${rblcShred}%<> for <#e8a800ff>${rblcDur} seconds<> (max ${rblcStacks} stacks)."
$i18n.en.sundered_sky.option = "Lightshield Strike: Your next instance of <#ff9028ff>physical damage<> against an enemy champion <$critIcon> <#d45656ff>critically strikes<> for <#e8a800ff>${ssDamage}% bonus damage<>, and <#60e84dff>heals you<> for <#60e84dff>${ssFlatHeal}<> + <#60e84dff>${ssPercentHeal}%<> of your <#60e84dff>missing health<> (<#e8a800ff>${ssOnHitCD} second<> cooldown per target)."
$i18n.en.radiant_sundered_sky.option = "Lightshield Strike: Your next instance of <#ff9028ff>physical damage<> against an enemy champion <$critIcon> <#d45656ff>critically strikes<> for <#e8a800ff>${rssDamage}% bonus damage<>, and <#60e84dff>heals you<> for <#60e84dff>${rssFlatHeal}<> + <#60e84dff>${rssPercentHeal}%<> of your <#60e84dff>missing health<> (<#e8a800ff>${rssOnHitCD} second<> cooldown per target)."
$i18n.en.echoes_of_helia.option = "Soul Siphon: Store <#e8a800ff>${eohConversion}%<> of the damage you deal or take as <#92dc7bff>Soul Charges<>, up to <#d8c9b3ff>${eohMinCap}<> - <#d8c9b3ff>${eohMaxCap}<> (based on <$levelIcon> <#d8c9b3ff>level<>). Upon landing an Ability on an enemy champion, consume all <#92dc7bff>Soul Charges<> and <#60e84dff>heal the nearest ally<> equal to the consumed amount."
$i18n.en.radiant_echoes_of_helia.option = "Soul Siphon: Store <#e8a800ff>${reohConversion}%<> of the damage you deal or take as <#92dc7bff>Soul Charges<>, up to <#d8c9b3ff>${reohMinCap}<> - <#d8c9b3ff>${reohMaxCap}<> (based on <$levelIcon> <#d8c9b3ff>level<>). Upon landing an Ability on an enemy champion, consume all <#92dc7bff>Soul Charges<> and <#60e84dff>heal the nearest ally<> equal to the consumed amount."

$i18n.en.sheen.option = "Spellblade: Landing an Ability on an enemy champion causes your next Attack to deal <#d8c9b3ff>${sheenMin}<> - <#d8c9b3ff>${sheenMax}<> (based on <$levelIcon> <#d8c9b3ff>level<>) as <#ff9028ff>bonus physical damage<> (<#e8a800ff>${sheenCd} second<> cooldown)."

$tfTemplate = "Spellblade: Landing an Ability on an enemy champion causes your next Attack to deal <#ff9028ff>{0}<> + <#ff9028ff>{1}%<> of your <$adIcon> <#ff9028ff>Attack Damage<> as <#ff9028ff>bonus physical damage<> (<#e8a800ff>{2} second<> cooldown)."
$i18n.en.trinity_force.option = $tfTemplate -f $tfFlat, $tfAdPct, $tfCd
$i18n.en.radiant_trinity_force.option = $tfTemplate -f $rtfFlat, $rtfAdPct, $rtfCd

$dndTemplate = "Spellblade: Landing an Ability on an enemy champion causes your next Attack to deal <#a974ffff>{0}<> + <#a974ffff>{1}%<> of your <$apIcon> <#a974ffff>Ability Power<> as <#a974ffff>bonus magic damage<> and <#60e84dff>heal you<> for <#a974ffff>{2}%<> of your <$apIcon> <#a974ffff>Ability Power<> and <#60e84dff>{3}%<> of your <$hpIcon> <#60e84dff>maximum health<> (<#e8a800ff>{4} second<> cooldown)."
$i18n.en.dusk_and_dawn.option = $dndTemplate -f $dndFlat, $dndApPct, $dndApHeal, $dndHpHeal, $dndCd
$i18n.en.radiant_dusk_and_dawn.option = $dndTemplate -f $rdndFlat, $rdndApPct, $rdndApHeal, $rdndHpHeal, $rdndCd

$bsTemplate = "Spellblade: Landing an Ability on an enemy champion causes your next Attack to deal <#d8c9b3ff>{0}<> - <#d8c9b3ff>{1}<> (based on <$levelIcon> <#d8c9b3ff>level<>) as <#a974ffff>bonus magic damage<> (<#e8a800ff>{2} second<> cooldown). If the target is a champion, increase their <#d94c49ff>damage taken<> by <#d94c49ff>{3}%<> for <#e8a800ff>{4} seconds<>."
$i18n.en.bloodsong.option = $bsTemplate -f $bsMin, $bsMax, $bsCd, $bsAmp, $bsDur
$i18n.en.radiant_bloodsong.option = $bsTemplate -f $rbsMin, $rbsMax, $rbsCd, $rbsAmp, $rbsDur

$lethEn = "Lethality: Ignore <#ffdd8eff>{0} <$armorIcon> armor<> when you deal damage to enemies."
$i18n.en.serrated_dirk.option = $lethEn -f $sdLeth
$hubEn = "$lethEn`n`nEminence: Scoring a <#e8a800ff>takedown<> on an enemy champion generates a permanent stack and grants <#ff9028ff>{1}<> (+{2} per stack) <#ff9028ff>bonus<> <$adIcon> <#ff9028ff>Attack Damage<> for <#e8a800ff>{3} seconds<>."
$i18n.en.hubris.option = $hubEn -f $hubLeth, $hubBase, $hubStack, $hubDur
$i18n.en.radiant_hubris.option = $hubEn -f $rhubLeth, $rhubBase, $rhubStack, $rhubDur
$bbEn = "$lethEn`n`nSabotage: Scoring a <#e8a800ff>takedown<> on an enemy champion grants <#92dc7bff>Sabotage<> for <#e8a800ff>{3} seconds<>, empowering your next Attack against a turret to deal <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> of your <$adIcon> <#ff9028ff>Attack Damage<> as <#ff9028ff>bonus physical damage<>."
$i18n.en.bastionbreaker.option = $bbEn -f $bbLeth, $bbFlat, $bbPct, $bbDur
$i18n.en.radiant_bastionbreaker.option = $bbEn -f $rbbLeth, $rbbFlat, $rbbPct, $rbbDur
$spfEn = "$lethEn`n`nShield Reaver: Dealing damage to an enemy champion with a shield deals <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> of your <$adIcon> <#ff9028ff>Attack Damage<> as <#ff9028ff>bonus physical damage<>."
$i18n.en.serpents_fang.option = $spfEn -f $spfLeth, $spfFlat, $spfPct
$i18n.en.radiant_serpents_fang.option = $spfEn -f $rspfLeth, $rspfFlat, $rspfPct

Write-Host "Done."
Write-Host "Updating Vietnamese text."

$i18n.vi.executioners_calling.option = "Vết thương chí mạng: Gây <#ff9028ff>sát thương vật lý<> lên tướng địch <#d94c49ff>giảm hồi máu của chúng ${execHeal}%<> trong <#e8a800ff>${execDur} giây<>."
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
$i18n.vi.shadowflame.option = "Hạt Tro: <#a974ffff>Sát thương phép<> của bạn <#e8a800ff>mạnh hơn 20%<> khi gây lên kẻ địch <#d94c49ff>dưới ${sfThreshold}% máu tối đa<>."
$i18n.vi.radiant_shadowflame.option = "Hạt Tro: <#a974ffff>Sát thương phép<> của bạn <#e8a800ff>mạnh hơn 20%<> khi gây lên kẻ địch <#d94c49ff>dưới ${rsfThreshold}% máu tối đa<>."
$i18n.vi.yun_tal_wildarrows.option = "Chí Mạng Tay Quen: Gây <#ff9028ff>sát thương vật lí<> sẽ tăng <#e8a800ff>${ytCrit}%<> <$critIcon> <#e8a800ff>tỉ lệ chí mạng<> vĩnh viễn, tối đa <#e8a800ff>${ytMaxCrit}%<>.`n`nChuyển Động Liên Hoàn: Khi tấn công, nhận <#ceff99ff>${ytFlurryAS}%<> <$asIcon> <#ceff99ff>tốc độ đánh<> trong <#e8a800ff>${ytDur} giây<> (hồi chiêu ${ytCd} giây)."
$i18n.vi.radiant_yun_tal_wildarrows.option = "Chí Mạng Tay Quen: Gây <#ff9028ff>sát thương vật lí<> sẽ tăng <#e8a800ff>${rytCrit}%<> <$critIcon> <#e8a800ff>tỉ lệ chí mạng<> vĩnh viễn, tối đa <#e8a800ff>${rytMaxCrit}%<>.`n`nChuyển Động Liên Hoàn: Khi tấn công, nhận <#ceff99ff>${rytFlurryAS}%<> <$asIcon> <#ceff99ff>tốc độ đánh<> trong <#e8a800ff>${rytDur} giây<> (hồi chiêu ${rytCd} giây)."
$i18n.vi.mortal_reminder.option = "Vết thương chí mạng: Gây <#ff9028ff>sát thương vật lý<> lên tướng địch <#d94c49ff>giảm hồi máu của chúng ${mrHeal}%<> trong <#e8a800ff>${mrDur} giây<>."
$i18n.vi.radiant_mortal_reminder.option = "Vết thương chí mạng: Gây <#ff9028ff>sát thương vật lý<> lên tướng địch <#d94c49ff>giảm hồi máu của chúng ${rmrHeal}%<> trong <#e8a800ff>${rmrDur} giây<>."
$i18n.vi.jaksho_the_protean.option = "Kiên cường: Nhận sát thương từ tướng địch sẽ nhận thêm <#ffdd8eff>${jakDefMult}% <$armorIcon> giáp<> và <#88ccffff>${jakMrMult}% <$mrIcon> kháng phép<> trong <#e8a800ff>${jakDur} giây<> (tối đa ${jakStacks} cộng dồn)."
$i18n.vi.radiant_jaksho_the_protean.option = "Kiên cường: Nhận sát thương từ tướng địch sẽ nhận thêm <#ffdd8eff>${rjakDefMult}% <$armorIcon> giáp<> và <#88ccffff>${rjakMrMult}% <$mrIcon> kháng phép<> trong <#e8a800ff>${rjakDur} giây<> (tối đa ${rjakStacks} cộng dồn)."
$i18n.vi.frozen_mallet.option = "Băng kết: Khi tấn công, gây <#d94c49ff>${fmSlow}% kiệt sức <> trong <#e8a800ff>${fmDur} giây<>."
$i18n.vi.radiant_frozen_mallet.option = "Băng kết: Khi tấn công, gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân và gây <#d94c49ff>${rfmSlow}% kiệt sức <> trong <#e8a800ff>${rfmDur} giây<>."
$i18n.vi.rylais_crystal_scepter.option = "Sương Giá: Khi gây sát thương kĩ năng, gây <#d94c49ff>${rcsSlow}% kiệt sức<> cho mục tiêu trúng đòn trong <#e8a800ff>${rcsDur} giây<>."
$i18n.vi.radiant_rylais_crystal_scepter.option = "Sương Giá: Khi gây sát thương kĩ năng, gây <#d94c49ff>${rrcsSlow}% kiệt sức<> cho mục tiêu trúng đòn trong <#e8a800ff>${rrcsDur} giây<>."
$i18n.vi.experimental_hexplate.option = "Tăng tốc: Nhận <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>giảm thời gian hồi chiêu<> cuối."
$i18n.vi.radiant_experimental_hexplate.option = "Tăng tốc: Nhận <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>giảm thời gian hồi chiêu<> cuối."
$i18n.vi.guinsoos_rageblade.option = "Thịnh nộ: Tấn công thường gây ra thêm <#a974ffff>${gbDmg} sát thương phép thuật<>.`n`nDồn dập: Khi tấn công, nhận <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>tốc độ đánh<> trong <#e8a800ff>${gbDur} giây<> (tối đa ${gbStacks} cộng dồn)."
$i18n.vi.radiant_guinsoos_rageblade.option = "Thịnh nộ: Tấn công thường gây ra thêm <#a974ffff>${rgbDmg} sát thương phép thuật<>.`n`nDồn dập: Khi tấn công, nhận <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>tốc độ đánh<> trong <#e8a800ff>${rgbDur} giây<> (tối đa ${rgbStacks} cộng dồn)."
$i18n.vi.wits_end.option = "Xé Toạc: Tấn công thường gây ra thêm <#a974ffff>${weDmg} sát thương phép thuật<>."
$i18n.vi.radiant_wits_end.option = "Xé Toạc: Tấn công thường gây ra thêm <#a974ffff>${rweDmg} sát thương phép thuật<>."
$i18n.vi.kraken_slayer.option = "Bắn Hạ: Mỗi đòn đánh thường thứ ba gây <#ff9028ff>${ksDmg} sát thương vật lí cộng thêm<>, tăng thêm tối đa <#ff9028ff>${ksBonus}%<> dựa trên <#60e84dff>máu đã mất<> của mục tiêu (đạt tối đa khi mục tiêu còn <#60e84dff>${ksThresh}%<> máu)."
$i18n.vi.radiant_kraken_slayer.option = "Bắn Hạ: Mỗi đòn đánh thường thứ ba gây <#ff9028ff>${rksDmg} sát thương vật lí cộng thêm<>, tăng thêm tối đa <#ff9028ff>${rksBonus}%<> dựa trên <#60e84dff>máu đã mất<> của mục tiêu (đạt tối đa khi mục tiêu còn <#60e84dff>${rksThresh}%<> máu)."
$i18n.vi.atmas_reckoning.option = "Cơ Bắp Cuồn Cuộn: Nhận <#e8a800ff>${arCrit}%<> <$critIcon> <#e8a800ff>tỉ lệ chí mạng<> cho mỗi <#60e84dff>${arHp}<> <$hpIcon> <#60e84dff>máu tối đa<>, tối đa <#e8a800ff>${arCap}%<>."
$i18n.vi.radiant_atmas_reckoning.option = "Cơ Bắp Cuồn Cuộn: Nhận <#e8a800ff>${rarCrit}%<> <$critIcon> <#e8a800ff>tỉ lệ chí mạng<> cho mỗi <#60e84dff>${rarHp}<> <$hpIcon> <#60e84dff>máu tối đa<>, tối đa <#e8a800ff>${rarCap}%<>."
$i18n.vi.lord_dominiks_regards.option = "Diệt Khổng Lồ: Gây thêm <#ff9028ff>${ldrPct}% sát thương<> cho mỗi <#60e84dff>${ldrHp}<> <$hpIcon> <#60e84dff>máu tối đa<> của mục tiêu, tối đa <#ff9028ff>${ldrMax}%<>."
$i18n.vi.radiant_lord_dominiks_regards.option = "Diệt Khổng Lồ: Gây thêm <#ff9028ff>${rldrPct}% sát thương<> cho mỗi <#60e84dff>${rldrHp}<> <$hpIcon> <#60e84dff>máu tối đa<> của mục tiêu, tối đa <#ff9028ff>${rldrMax}%<>."
$i18n.vi.blackfire_torch.option = "Tội ác: Kĩ năng trúng sẽ tăng <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>SMPT<> trong <#e8a800ff>${bftDur} giây<> (tối đa ${bftStacks} cộng dồn)."
$i18n.vi.radiant_blackfire_torch.option = "Tội ác: Kĩ năng trúng sẽ tăng <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>SMPT<> trong <#e8a800ff>${rbftDur} giây<> (tối đa ${rbftStacks} cộng dồn)."
$i18n.vi.blade_of_the_ruined_king.option = "Nanh vuốt sương mù: Khi tấn công, gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#d94c49ff>${borkPct}% máu hiện tại của mục tiêu<>. Tối đa <#ff9028ff>${borkCap} sát thương vật lí<> lên lính và quái vật."
$i18n.vi.radiant_blade_of_the_ruined_king.option = "Nanh vuốt sương mù: Khi tấn công, gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#d94c49ff>${rborkPct}% máu hiện tại của mục tiêu<>. Tối đa <#ff9028ff>${rborkCap} sát thương vật lí<> lên lính và quái vật."
$i18n.vi.deathblade.option = "Cường hóa: Tăng <$adIcon> <#ff9028ff>SMCK<> của bản thân thêm <#ff9028ff>${dbMult}%<>."
$i18n.vi.radiant_deathblade.option = "Cường hóa: Tăng <$adIcon> <#ff9028ff>SMCK<> của bản thân thêm <#ff9028ff>${rdbMult}%<>."
$i18n.vi.deaths_dance.option = "Phớt Lờ Đau Đớn: <#e8a800ff>${ddDelay}%<> sát thương nhận vào được tích trữ và gây lại cho bạn theo thời gian dưới dạng <#d94c49ff>sát thương vật lí<> (tối đa <#60e84dff>${ddBurnCap}%<> <$hpIcon> <#60e84dff>máu tối đa<> mỗi giây).`n`nCự Tuyệt: Tham gia hạ gục một tướng địch sẽ xóa lượng sát thương tích trữ còn lại và <#60e84dff>hồi<> cho bạn <#60e84dff>${ddFlatHeal}<> + <#60e84dff>${ddHeal}%<> <#60e84dff>máu đã mất<>."
$i18n.vi.radiant_deaths_dance.option = "Phớt Lờ Đau Đớn: <#e8a800ff>${rddDelay}%<> sát thương nhận vào được tích trữ và gây lại cho bạn theo thời gian dưới dạng <#d94c49ff>sát thương vật lí<> (tối đa <#60e84dff>${rddBurnCap}%<> <$hpIcon> <#60e84dff>máu tối đa<> mỗi giây).`n`nCự Tuyệt: Tham gia hạ gục một tướng địch sẽ xóa lượng sát thương tích trữ còn lại và <#60e84dff>hồi<> cho bạn <#60e84dff>${rddFlatHeal}<> + <#60e84dff>${rddHeal}%<> <#60e84dff>máu đã mất<>."
$i18n.vi.rabadons_deathcap.option = "Hạt nhân: Tăng <$apIcon> <#a974ffff>SMPT<> của bản thân thêm <#a974ffff>${rabMult}%<>."
$i18n.vi.radiant_rabadons_deathcap.option = "Hạt nhân: Tăng <$apIcon> <#a974ffff>SMPT<> của bản thân thêm <#a974ffff>${radRabMult}%<>."

$mbIllusionVi = "Ảo ảnh: Nhận <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Lực Thích Ứng<>. Với mỗi <$forceIcon> <#d48294ff>Lực Thích Ứng<> tăng <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>SMCK<> hoặc <#a974ffff>1<> <$apIcon> <#a974ffff>SMPT<>, tùy vào bên nào cao hơn.`n`nMờ ảo: Khi kết liễu tướng địch, nhận <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>tốc độ di chuyển<> trong <#e8a800ff>{2} giây<>."
$i18n.vi.mirage_blade.option = $mbIllusionVi -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.vi.radiant_mirage_blade.option = $mbIllusionVi -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$dtsPierceVi = "Xuyên Thấu: Nhận <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Lực Thích Ứng<>. Với mỗi <$forceIcon> <#d48294ff>Lực Thích Ứng<> tăng <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>SMCK<> hoặc <#a974ffff>1<> <$apIcon> <#a974ffff>SMPT<>, tùy vào bên nào cao hơn.`n`nĐiểm Lý Tưởng: Gây tối đa <#e8a800ff>{1}% sát thương cộng thêm<> lên tướng địch dựa trên khoảng cách (hiệu quả tối đa ở <#ff86c2ff>{2}<> <$rangeIcon> )."
$i18n.vi.diamond_tipped_spear.option = $dtsPierceVi -f $dtsForce, $dtsPct, $dtsDist
$i18n.vi.radiant_diamond_tipped_spear.option = $dtsPierceVi -f $rdtsForce, $rdtsPct, $rdtsDist
$zekAuraVi = "Hào Quang: Cấp <#d48294ff>{0}<> <$forceIcon> <#d48294ff>Lực Thích Ứng<> và <#b7462dff>{2}%<> <$vampIcon> <#b7462dff>hút máu<> cho mọi tướng đồng minh trong phạm vi <#ff86c2ff>{1}<> <$rangeIcon> ."
$i18n.vi.zekes_herald.option = $zekAuraVi -f $zekForce, $zekDist, $zekVamp
$i18n.vi.radiant_zekes_herald.option = $zekAuraVi -f $rzekForce, $rzekDist, $rzekVamp
$ssBullseyeVi = "Hồng Tâm: Khi gây sát thương cho tướng địch, gây <#a974ffff>{0} sát thương phép cộng thêm<> (hồi chiêu <#e8a800ff>{1} giây<>)."
$i18n.vi.scouts_slingshot.option = $ssBullseyeVi -f $ssDmg, $ssCd
$litSufferingVi = "Thống Khổ: Khi gây sát thương kĩ năng, thiêu đốt kẻ địch, khiến chúng chịu <#d94c49ff>{0}% máu tối đa của chúng<> dưới dạng <#a974ffff>sát thương phép<> trong <#e8a800ff>{1} giây<>. Tối đa <#a974ffff>{2} sát thương phép<> mỗi nhịp lên lính và quái vật."
$i18n.vi.liandrys_torment.option = $litSufferingVi -f $litHp, $litDur, $litCap
$i18n.vi.radiant_liandrys_torment.option = $litSufferingVi -f $rlitHp, $rlitDur, $rlitCap
$i18n.vi.spirit_visage.option = "Sức sống: Tăng khả năng <#60e84dff>nhận hồi máu<> thêm <#60e84dff>${svHeal}%<>."
$i18n.vi.radiant_spirit_visage.option = "Sức sống: Tăng khả năng <#60e84dff>nhận hồi máu<> thêm <#60e84dff>${rsvHeal}%<>."
$i18n.vi.unending_despair.option = "Thống khổ: Kĩ năng trúng bạn sẽ hồi máu một lượng tương ứng <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> cho bản thân."
$i18n.vi.radiant_unending_despair.option = "Thống khổ: Kĩ năng trúng bạn sẽ hồi máu một lượng tương ứng <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> cho bản thân."
$i18n.vi.protoplasm_harness.option = "Vững chãi: Máu rơi xuống dưới <#d94c49ff>${phThreshold}%<> sẽ nhận <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, được xem là <#60e84dff>máu cộng thêm<> trong <#e8a800ff>${phDur} giây<> và <#60e84dff>hồi máu<> bằng phân nữa lượng đó (trong ${phCd} giây.)."
$i18n.vi.radiant_protoplasm_harness.option = "Vững chãi: Máu rơi xuống dưới <#d94c49ff>${rphThreshold}%<> sẽ nhận <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, được xem là <#60e84dff>máu cộng thêm<> trong <#e8a800ff>${rphDur} giây<> và <#60e84dff>hồi máu<> bằng phân nữa lượng đó (trong ${rphCd} giây.)."
$i18n.vi.terminus.option = "Đối lập: Khi tấn công, nhận luân phiên <#ffdd8eff>${tArmorPen}% <$armorPenIcon> xuyên giáp<> hoặc <#88ccffff>${tMagicPen}% <$magicPenIcon> xuyên kháng phép<> trong <#e8a800ff>${tDur} giây<> (tối đa ${tStacks} cộng dồn cho mỗi loại)."
$i18n.vi.radiant_terminus.option = "Đối lập: Khi tấn công, nhận luân phiên <#ffdd8eff>${rtArmorPen}% <$armorPenIcon> xuyên giáp<> hoặc <#88ccffff>${rtMagicPen}% <$magicPenIcon> xuyên kháng phép<> trong <#e8a800ff>${rtDur} giây<> (tối đa ${rtStacks} cộng dồn cho mỗi loại)."
$i18n.vi.collector.option = "Về Với Cát Bụi: Gây sát thương lên tướng địch dưới <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>Máu<> sẽ lập tức <#d94c49ff>kết liễu<> chúng."
$i18n.vi.radiant_collector.option = "Về Với Cát Bụi: Gây sát thương lên tướng địch dưới <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>Máu<> sẽ lập tức <#d94c49ff>kết liễu<> chúng."
$i18n.vi.heartsteel.option = "Trái Tim Sắt Đá: Mỗi <#e8a800ff>${hsCd} giây<>, đòn đánh tiếp theo của bạn gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, đồng thời nhận vĩnh viễn <#60e84dff>${hsBonusHpPct}%<> sát thương đó dưới dạng <#60e84dff>máu cộng thêm<>."
$i18n.vi.radiant_heartsteel.option = "Trái Tim Sắt Đá: Mỗi <#e8a800ff>${rhsCd} giây<>, đòn đánh tiếp theo của bạn gây thêm <#ff9028ff>sát thương vật lí<> tương ứng <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> <$hpIcon> <#60e84dff>máu tối đa<> của bản thân, đồng thời nhận vĩnh viễn <#60e84dff>${rhsBonusHpPct}%<> sát thương đó dưới dạng <#60e84dff>máu cộng thêm<>."
$i18n.vi.spear_of_shojin.option = "Ý Chí Tập Trung: Kĩ năng trúng tướng địch sẽ tăng <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>SMCK<> trong <#e8a800ff>${sosDur} giây<> (tối đa ${sosStacks} cộng dồn)."
$i18n.vi.radiant_spear_of_shojin.option = "Ý Chí Tập Trung: Kĩ năng trúng tướng địch sẽ tăng <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>SMCK<> trong <#e8a800ff>${rsosDur} giây<> (tối đa ${rsosStacks} cộng dồn)."
$i18n.vi.warmogs_armor.option = "Trái Tim Warmog: Hồi phục <#60e84dff>${waHeal}%<> <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>máu tối đa<> của bạn mỗi giây và nhận <#ffffffff>${waMs}%<> <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>tốc độ di chuyển<> nếu không nhận sát thương trong <#e8a800ff>${waDur} giây<> gần đây."
$i18n.vi.radiant_warmogs_armor.option = "Trái Tim Warmog: Hồi phục <#60e84dff>${rwaHeal}%<> <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>máu tối đa<> của bạn mỗi giây và nhận <#ffffffff>${rwaMs}%<> <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>tốc độ di chuyển<> nếu không nhận sát thương trong <#e8a800ff>${rwaDur} giây<> gần đây."
$i18n.vi.stormrazor.option = "Tích Điện: Di chuyển và gây <#ff9028ff>sát thương vật lí<> tạo ra điểm <#e8a800ff>Tích Điện<>, tối đa <#e8a800ff>${srStacks}<>.`n`nTia Sét: Khi <#e8a800ff>Tích Điện<> đầy, đòn <#ff9028ff>sát thương vật lí<> tiếp theo gây <#a974ffff>${srDmg} sát thương phép cộng thêm<> và tăng <#ffffffff>${srMs}%<> <$speedIcon> <#ffffffff>tốc độ di chuyển<> trong <#e8a800ff>${srDur} giây<>."
$i18n.vi.radiant_stormrazor.option = "Tích Điện: Di chuyển và gây <#ff9028ff>sát thương vật lí<> tạo ra điểm <#e8a800ff>Tích Điện<>, tối đa <#e8a800ff>${rsrStacks}<>.`n`nTia Sét: Khi <#e8a800ff>Tích Điện<> đầy, đòn <#ff9028ff>sát thương vật lí<> tiếp theo gây <#a974ffff>${rsrDmg} sát thương phép cộng thêm<> và tăng <#ffffffff>${rsrMs}%<> <$speedIcon> <#ffffffff>tốc độ di chuyển<> trong <#e8a800ff>${rsrDur} giây<>."

$i18n.vi.black_cleaver.option = "Xẻ Thịt: Gây <#ff9028ff>sát thương vật lý<> lên tướng địch <#d94c49ff>giảm <$armorIcon> <#ffdd8eff>giáp<> của chúng ${bcShred}%<> trong <#e8a800ff>${bcDur} giây<> (tối đa ${bcStacks} cộng dồn)."
$i18n.vi.radiant_black_cleaver.option = "Xẻ Thịt: Gây <#ff9028ff>sát thương vật lý<> lên tướng địch <#d94c49ff>giảm <$armorIcon> <#ffdd8eff>giáp<> của chúng ${rbcShred}%<> trong <#e8a800ff>${rbcDur} giây<> (tối đa ${rbcStacks} cộng dồn)."
$i18n.vi.bloodletters_curse.option = "Phân Rã: Gây <#a974ffff>sát thương phép<> lên tướng địch <#d94c49ff>giảm <$mrIcon> <#88ccffff>kháng phép<> của chúng ${blcShred}%<> trong <#e8a800ff>${blcDur} giây<> (tối đa ${blcStacks} cộng dồn)."
$i18n.vi.radiant_bloodletters_curse.option = "Phân Rã: Gây <#a974ffff>sát thương phép<> lên tướng địch <#d94c49ff>giảm <$mrIcon> <#88ccffff>kháng phép<> của chúng ${rblcShred}%<> trong <#e8a800ff>${rblcDur} giây<> (tối đa ${rblcStacks} cộng dồn)."
$i18n.vi.sundered_sky.option = "Đòn Khiên Sáng: Đòn <#ff9028ff>sát thương vật lí<> tiếp theo của bạn lên một tướng địch sẽ <$critIcon> <#d45656ff>chí mạng<>, gây <#e8a800ff>${ssDamage}% sát thương cộng thêm<>, và <#60e84dff>hồi máu cho bạn<> một lượng bằng <#60e84dff>${ssFlatHeal}<> + <#60e84dff>${ssPercentHeal}%<> <#60e84dff>máu đã mất<> (<#e8a800ff>${ssOnHitCD} giây<> hồi chiêu mỗi mục tiêu)."
$i18n.vi.radiant_sundered_sky.option = "Đòn Khiên Sáng: Đòn <#ff9028ff>sát thương vật lí<> tiếp theo của bạn lên một tướng địch sẽ <$critIcon> <#d45656ff>chí mạng<>, gây <#e8a800ff>${rssDamage}% sát thương cộng thêm<>, và <#60e84dff>hồi máu cho bạn<> một lượng bằng <#60e84dff>${rssFlatHeal}<> + <#60e84dff>${rssPercentHeal}%<> <#60e84dff>máu đã mất<> (<#e8a800ff>${rssOnHitCD} giây<> hồi chiêu mỗi mục tiêu)."
$i18n.vi.echoes_of_helia.option = "Hút Hồn: Tích trữ <#e8a800ff>${eohConversion}%<> sát thương bạn gây ra dưới dạng <#92dc7bff>Hồn Lực<>, tối đa <$levelIcon> <#d8c9b3ff>${eohMinCap}<> - <#d8c9b3ff>${eohMaxCap}<> (tăng theo <#d8c9b3ff>cấp độ<>). Kĩ năng trúng tướng địch sẽ tiêu hết <#92dc7bff>Hồn Lực<>, <#60e84dff>hồi máu cho đồng minh gần nhất<> một lượng bằng số đã tiêu."
$i18n.vi.radiant_echoes_of_helia.option = "Hút Hồn: Tích trữ <#e8a800ff>${reohConversion}%<> sát thương bạn gây ra dưới dạng <#92dc7bff>Hồn Lực<>, tối đa <$levelIcon> <#d8c9b3ff>${reohMinCap}<> - <#d8c9b3ff>${reohMaxCap}<> (tăng theo <#d8c9b3ff>cấp độ<>). Kĩ năng trúng tướng địch sẽ tiêu hết <#92dc7bff>Hồn Lực<>, <#60e84dff>hồi máu cho đồng minh gần nhất<> một lượng bằng số đã tiêu."

$i18n.vi.sheen.option = "Kiếm Phép: Kĩ năng trúng tướng địch khiến đòn đánh tiếp theo của bạn gây <#d8c9b3ff>${sheenMin}<> - <#d8c9b3ff>${sheenMax}<> (dựa theo <$levelIcon> <#d8c9b3ff>cấp độ<>) dưới dạng <#ff9028ff>sát thương vật lí cộng thêm<> (hồi chiêu <#e8a800ff>${sheenCd} giây<>)."

$tfTemplateVi = "Kiếm Phép: Kĩ năng trúng tướng địch khiến đòn đánh tiếp theo của bạn gây <#ff9028ff>{0}<> + <#ff9028ff>{1}%<> <$adIcon> <#ff9028ff>SMCK<> dưới dạng <#ff9028ff>sát thương vật lí cộng thêm<> (hồi chiêu <#e8a800ff>{2} giây<>)."
$i18n.vi.trinity_force.option = $tfTemplateVi -f $tfFlat, $tfAdPct, $tfCd
$i18n.vi.radiant_trinity_force.option = $tfTemplateVi -f $rtfFlat, $rtfAdPct, $rtfCd

$dndTemplateVi = "Kiếm Phép: Kĩ năng trúng tướng địch khiến đòn đánh tiếp theo của bạn gây <#a974ffff>{0}<> + <#a974ffff>{1}%<> <$apIcon> <#a974ffff>SMPT<> dưới dạng <#a974ffff>sát thương phép cộng thêm<> và <#60e84dff>hồi máu cho bạn<> <#a974ffff>{2}%<> <$apIcon> <#a974ffff>SMPT<> và <#60e84dff>{3}%<> <$hpIcon> <#60e84dff>máu tối đa<> (hồi chiêu <#e8a800ff>{4} giây<>)."
$i18n.vi.dusk_and_dawn.option = $dndTemplateVi -f $dndFlat, $dndApPct, $dndApHeal, $dndHpHeal, $dndCd
$i18n.vi.radiant_dusk_and_dawn.option = $dndTemplateVi -f $rdndFlat, $rdndApPct, $rdndApHeal, $rdndHpHeal, $rdndCd

$bsTemplateVi = "Kiếm Phép: Kĩ năng trúng tướng địch khiến đòn đánh tiếp theo của bạn gây <#d8c9b3ff>{0}<> - <#d8c9b3ff>{1}<> (dựa theo <$levelIcon> <#d8c9b3ff>cấp độ<>) dưới dạng <#a974ffff>sát thương phép cộng thêm<> (hồi chiêu <#e8a800ff>{2} giây<>). Nếu mục tiêu là tướng, tăng <#d94c49ff>sát thương chúng phải nhận<> thêm <#d94c49ff>{3}%<> trong <#e8a800ff>{4} giây<>."
$i18n.vi.bloodsong.option = $bsTemplateVi -f $bsMin, $bsMax, $bsCd, $bsAmp, $bsDur
$i18n.vi.radiant_bloodsong.option = $bsTemplateVi -f $rbsMin, $rbsMax, $rbsCd, $rbsAmp, $rbsDur

$lethVi = "Xuyên Giáp Trắng: Bỏ qua <#ffdd8eff>{0} <$armorIcon> giáp<> khi gây sát thương lên kẻ địch."
$i18n.vi.serrated_dirk.option = $lethVi -f $sdLeth
$hubVi = "$lethVi`n`nUy Danh: Khi tham gia hạ gục một tướng địch, tạo một cộng dồn vĩnh viễn và nhận <#ff9028ff>{1}<> (+{2} mỗi cộng dồn) <$adIcon> <#ff9028ff>SMCK cộng thêm<> trong <#e8a800ff>{3} giây<>."
$i18n.vi.hubris.option = $hubVi -f $hubLeth, $hubBase, $hubStack, $hubDur
$i18n.vi.radiant_hubris.option = $hubVi -f $rhubLeth, $rhubBase, $rhubStack, $rhubDur
$bbVi = "$lethVi`n`nPhá Hoại: Tham gia hạ gục một tướng địch sẽ trao <#92dc7bff>Phá Hoại<> trong <#e8a800ff>{3} giây<>, tăng cường đòn Đánh tiếp theo của bạn lên trụ để gây <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> <$adIcon> <#ff9028ff>SMCK<> dưới dạng <#ff9028ff>sát thương vật lí cộng thêm<>."
$i18n.vi.bastionbreaker.option = $bbVi -f $bbLeth, $bbFlat, $bbPct, $bbDur
$i18n.vi.radiant_bastionbreaker.option = $bbVi -f $rbbLeth, $rbbFlat, $rbbPct, $rbbDur
$spfVi = "$lethVi`n`nKẻ Cắt Khiên: Gây sát thương lên tướng địch đang có lá chắn sẽ gây thêm <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> <$adIcon> <#ff9028ff>SMCK<> dưới dạng <#ff9028ff>sát thương vật lí cộng thêm<>."
$i18n.vi.serpents_fang.option = $spfVi -f $spfLeth, $spfFlat, $spfPct
$i18n.vi.radiant_serpents_fang.option = $spfVi -f $rspfLeth, $rspfFlat, $rspfPct

Write-Host "Done."
Write-Host "Updating Chinese (Simplified) text."

$i18n.'zh-hans'.executioners_calling.option = "重伤：对敌方英雄造成<#ff9028ff>物理伤害<>会使其<#d94c49ff>治疗效果降低${execHeal}%<>，持续 <#e8a800ff>${execDur}秒<>。"
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
$i18n.'zh-hans'.shadowflame.option = "余烬绽放：你的<#a974ffff>魔法伤害<>对<#d94c49ff>最大生命值低于${sfThreshold}%<>的敌人<#e8a800ff>提高20%<>。"
$i18n.'zh-hans'.radiant_shadowflame.option = "余烬绽放：你的<#a974ffff>魔法伤害<>对<#d94c49ff>最大生命值低于${rsfThreshold}%<>的敌人<#e8a800ff>提高20%<>。"
$i18n.'zh-hans'.yun_tal_wildarrows.option = "熟能生巧：造成<#ff9028ff>物理伤害<>时永久获得 <#e8a800ff>${ytCrit}%<> <$critIcon> <#e8a800ff>暴击几率<>，最多 <#e8a800ff>${ytMaxCrit}%<>。`n`n疾风骤雨：普通攻击时，获得 <#ceff99ff>${ytFlurryAS}%<> <$asIcon> <#ceff99ff>攻击速度<>，持续 <#e8a800ff>${ytDur}秒<>（冷却${ytCd}秒）。"
$i18n.'zh-hans'.radiant_yun_tal_wildarrows.option = "熟能生巧：造成<#ff9028ff>物理伤害<>时永久获得 <#e8a800ff>${rytCrit}%<> <$critIcon> <#e8a800ff>暴击几率<>，最多 <#e8a800ff>${rytMaxCrit}%<>。`n`n疾风骤雨：普通攻击时，获得 <#ceff99ff>${rytFlurryAS}%<> <$asIcon> <#ceff99ff>攻击速度<>，持续 <#e8a800ff>${rytDur}秒<>（冷却${rytCd}秒）。"
$i18n.'zh-hans'.mortal_reminder.option = "重伤：对敌方英雄造成<#ff9028ff>物理伤害<>会使其<#d94c49ff>治疗效果降低${mrHeal}%<>，持续 <#e8a800ff>${mrDur}秒<>。"
$i18n.'zh-hans'.radiant_mortal_reminder.option = "重伤：对敌方英雄造成<#ff9028ff>物理伤害<>会使其<#d94c49ff>治疗效果降低${rmrHeal}%<>，持续 <#e8a800ff>${rmrDur}秒<>。"
$i18n.'zh-hans'.jaksho_the_protean.option = "复原力：受到敌方英雄伤害时，获得 <#ffdd8eff>${jakDefMult}% <$armorIcon> 护甲<>和 <#88ccffff>${jakMrMult}% <$mrIcon> 魔法抗性<>，持续 <#e8a800ff>${jakDur}秒<>（最多叠加${jakStacks}层）。"
$i18n.'zh-hans'.radiant_jaksho_the_protean.option = "复原力：受到敌方英雄伤害时，获得 <#ffdd8eff>${rjakDefMult}% <$armorIcon> 护甲<>和 <#88ccffff>${rjakMrMult}% <$mrIcon> 魔法抗性<>，持续 <#e8a800ff>${rjakDur}秒<>（最多叠加${rjakStacks}层）。"
$i18n.'zh-hans'.frozen_mallet.option = "冰寒：普通攻击会施加 <#d94c49ff>${fmSlow}%减速<>，持续 <#e8a800ff>${fmDur}秒<>。"
$i18n.'zh-hans'.radiant_frozen_mallet.option = "冰寒：普通攻击造成 <#ff9028ff>额外物理伤害<>，数值为 <#ff9028ff>${rfmFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rfmHpPct}%<>，并施加 <#d94c49ff>${rfmSlow}%减速<>，持续 <#e8a800ff>${rfmDur}秒<>。"
$i18n.'zh-hans'.rylais_crystal_scepter.option = "凛霜：造成技能伤害时，对命中的单位施加 <#d94c49ff>${rcsSlow}%减速<>，持续 <#e8a800ff>${rcsDur}秒<>。"
$i18n.'zh-hans'.radiant_rylais_crystal_scepter.option = "凛霜：造成技能伤害时，对命中的单位施加 <#d94c49ff>${rrcsSlow}%减速<>，持续 <#e8a800ff>${rrcsDur}秒<>。"
$i18n.'zh-hans'.experimental_hexplate.option = "过载：终极技能获得 <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>冷却缩减<>。"
$i18n.'zh-hans'.radiant_experimental_hexplate.option = "过载：终极技能获得 <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>冷却缩减<>。"
$i18n.'zh-hans'.guinsoos_rageblade.option = "怨怒：普通攻击造成 <#a974ffff>${gbDmg}点额外魔法伤害<>。`n`n沸腾打击：普通攻击时获得 <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>攻击速度<>，持续 <#e8a800ff>${gbDur}秒<>（最多叠加${gbStacks}层）。"
$i18n.'zh-hans'.radiant_guinsoos_rageblade.option = "怨怒：普通攻击造成 <#a974ffff>${rgbDmg}点额外魔法伤害<>。`n`n沸腾打击：普通攻击时获得 <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>攻击速度<>，持续 <#e8a800ff>${rgbDur}秒<>（最多叠加${rgbStacks}层）。"
$i18n.'zh-hans'.wits_end.option = "喧争：普通攻击造成 <#a974ffff>${weDmg}点额外魔法伤害<>。"
$i18n.'zh-hans'.radiant_wits_end.option = "喧争：普通攻击造成 <#a974ffff>${rweDmg}点额外魔法伤害<>。"
$i18n.'zh-hans'.kraken_slayer.option = "放倒它：每第三次普通攻击造成 <#ff9028ff>${ksDmg}点额外物理伤害<>，并根据目标的<#60e84dff>已损失生命值<>最多提高 <#ff9028ff>${ksBonus}%<>（目标生命值低至 <#60e84dff>${ksThresh}%<> 时达到最大加成）。"
$i18n.'zh-hans'.radiant_kraken_slayer.option = "放倒它：每第三次普通攻击造成 <#ff9028ff>${rksDmg}点额外物理伤害<>，并根据目标的<#60e84dff>已损失生命值<>最多提高 <#ff9028ff>${rksBonus}%<>（目标生命值低至 <#60e84dff>${rksThresh}%<> 时达到最大加成）。"
$i18n.'zh-hans'.atmas_reckoning.option = "大手：每拥有 <#60e84dff>${arHp}点<> <$hpIcon> <#60e84dff>最大生命值<>，便获得 <#e8a800ff>${arCrit}%<> <$critIcon> <#e8a800ff>暴击几率<>，最多 <#e8a800ff>${arCap}%<>。"
$i18n.'zh-hans'.radiant_atmas_reckoning.option = "大手：每拥有 <#60e84dff>${rarHp}点<> <$hpIcon> <#60e84dff>最大生命值<>，便获得 <#e8a800ff>${rarCrit}%<> <$critIcon> <#e8a800ff>暴击几率<>，最多 <#e8a800ff>${rarCap}%<>。"
$i18n.'zh-hans'.lord_dominiks_regards.option = "巨人杀手：目标每拥有 <#60e84dff>${ldrHp}点<> <$hpIcon> <#60e84dff>最大生命值<>，便造成 <#ff9028ff>${ldrPct}%<> 额外伤害，最多 <#ff9028ff>${ldrMax}%<>。"
$i18n.'zh-hans'.radiant_lord_dominiks_regards.option = "巨人杀手：目标每拥有 <#60e84dff>${rldrHp}点<> <$hpIcon> <#60e84dff>最大生命值<>，便造成 <#ff9028ff>${rldrPct}%<> 额外伤害，最多 <#ff9028ff>${rldrMax}%<>。"
$i18n.'zh-hans'.blackfire_torch.option = "邪焰：技能命中时获得 <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>法术强度<>，持续 <#e8a800ff>${bftDur}秒<>（最多叠加${bftStacks}层）。"
$i18n.'zh-hans'.radiant_blackfire_torch.option = "邪焰：技能命中时获得 <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>法术强度<>，持续 <#e8a800ff>${rbftDur}秒<>（最多叠加${rbftStacks}层）。"
$i18n.'zh-hans'.blade_of_the_ruined_king.option = "雾之锋：普通攻击造成相当于目标<#d94c49ff>当前生命值${borkPct}%<>的<#ff9028ff>额外物理伤害<>。对小兵和野怪最多造成 <#ff9028ff>${borkCap}点物理伤害<>。"
$i18n.'zh-hans'.radiant_blade_of_the_ruined_king.option = "雾之锋：普通攻击造成相当于目标<#d94c49ff>当前生命值${rborkPct}%<>的<#ff9028ff>额外物理伤害<>。对小兵和野怪最多造成 <#ff9028ff>${rborkCap}点物理伤害<>。"
$i18n.'zh-hans'.deathblade.option = "顶峰：你的总 <$adIcon> <#ff9028ff>攻击力<>提升 <#ff9028ff>${dbMult}%<>。"
$i18n.'zh-hans'.radiant_deathblade.option = "顶峰：你的总 <$adIcon> <#ff9028ff>攻击力<>提升 <#ff9028ff>${rdbMult}%<>。"
$i18n.'zh-hans'.deaths_dance.option = "无视痛苦：受到伤害的 <#e8a800ff>${ddDelay}%<> 会被储存，并随时间作为 <#d94c49ff>额外物理伤害<> 返还给你（每秒最多为你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${ddBurnCap}%<>）。`n`n拒止：参与击杀敌方英雄会清除剩余的储存伤害，并为你<#60e84dff>回复<> <#60e84dff>${ddFlatHeal}<> + <#60e84dff>${ddHeal}%<> 的<#60e84dff>已损失生命值<>。"
$i18n.'zh-hans'.radiant_deaths_dance.option = "无视痛苦：受到伤害的 <#e8a800ff>${rddDelay}%<> 会被储存，并随时间作为 <#d94c49ff>额外物理伤害<> 返还给你（每秒最多为你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rddBurnCap}%<>）。`n`n拒止：参与击杀敌方英雄会清除剩余的储存伤害，并为你<#60e84dff>回复<> <#60e84dff>${rddFlatHeal}<> + <#60e84dff>${rddHeal}%<> 的<#60e84dff>已损失生命值<>。"
$i18n.'zh-hans'.rabadons_deathcap.option = "魔法乐章：你的总 <$apIcon> <#a974ffff>法术强度<>提升 <#a974ffff>${rabMult}%<>。"
$i18n.'zh-hans'.radiant_rabadons_deathcap.option = "魔法乐章：你的总 <$apIcon> <#a974ffff>法术强度<>提升 <#a974ffff>${radRabMult}%<>。"

$mbIllusionZh = "幻象：获得 <#d48294ff>{0}<> <$forceIcon> <#d48294ff>自适应之力<>。每点 <$forceIcon> <#d48294ff>自适应之力<>会根据较高者提供 <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>攻击力<>或 <#a974ffff>1<> <$apIcon> <#a974ffff>法术强度<>。`n`n模糊：击杀时获得 <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>移动速度<>，持续 <#e8a800ff>{2}秒<>。"
$i18n.'zh-hans'.mirage_blade.option = $mbIllusionZh -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.'zh-hans'.radiant_mirage_blade.option = $mbIllusionZh -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$dtsPierceZh = "穿刺：获得 <#d48294ff>{0}<> <$forceIcon> <#d48294ff>自适应之力<>。每点 <$forceIcon> <#d48294ff>自适应之力<>会根据较高者提供 <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>攻击力<>或 <#a974ffff>1<> <$apIcon> <#a974ffff>法术强度<>。`n`n最佳距离：根据距离对敌方英雄造成最多 <#e8a800ff>{1}% 额外伤害<>（在 <#ff86c2ff>{2} <$rangeIcon> 射程<>时效果最大）。"
$i18n.'zh-hans'.diamond_tipped_spear.option = $dtsPierceZh -f $dtsForce, $dtsPct, $dtsDist
$i18n.'zh-hans'.radiant_diamond_tipped_spear.option = $dtsPierceZh -f $rdtsForce, $rdtsPct, $rdtsDist
$zekAuraZh = "光环：为 <#ff86c2ff>{1} <$rangeIcon> 射程<>范围内的所有友方英雄提供 <#d48294ff>{0}<> <$forceIcon> <#d48294ff>自适应之力<> 和 <#b7462dff>{2}%<> <$vampIcon> <#b7462dff>全能吸血<>。"
$i18n.'zh-hans'.zekes_herald.option = $zekAuraZh -f $zekForce, $zekDist, $zekVamp
$i18n.'zh-hans'.radiant_zekes_herald.option = $zekAuraZh -f $rzekForce, $rzekDist, $rzekVamp
$ssBullseyeZh = "靶心：对敌方英雄造成伤害时，额外造成 <#a974ffff>{0} 点魔法伤害<>（冷却 <#e8a800ff>{1}秒<>）。"
$i18n.'zh-hans'.scouts_slingshot.option = $ssBullseyeZh -f $ssDmg, $ssCd
$litSufferingZh = "苦难：造成技能伤害时点燃敌人，使其在 <#e8a800ff>{1}秒<>内受到相当于<#d94c49ff>其{0}%最大生命值<>的<#a974ffff>魔法伤害<>。对小兵和野怪每跳最多造成 <#a974ffff>{2}点魔法伤害<>。"
$i18n.'zh-hans'.liandrys_torment.option = $litSufferingZh -f $litHp, $litDur, $litCap
$i18n.'zh-hans'.radiant_liandrys_torment.option = $litSufferingZh -f $rlitHp, $rlitDur, $rlitCap
$i18n.'zh-hans'.spirit_visage.option = "无拘活力：受到的所有<#60e84dff>治疗效果<>提升 <#60e84dff>${svHeal}%<>。"
$i18n.'zh-hans'.radiant_spirit_visage.option = "无拘活力：受到的所有<#60e84dff>治疗效果<>提升 <#60e84dff>${rsvHeal}%<>。"
$i18n.'zh-hans'.unending_despair.option = "苦楚：技能命中时，回复 <#60e84dff>${udFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${udHpPct}%<> 的生命值。"
$i18n.'zh-hans'.radiant_unending_despair.option = "苦楚：技能命中时，回复 <#60e84dff>${rudFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rudHpPct}%<> 的生命值。"
$i18n.'zh-hans'.protoplasm_harness.option = "筑防：<#d94c49ff>生命值降至${phThreshold}%以下<>时，获得 <#60e84dff>${phFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${phHpPct}%<> 作为<#60e84dff>额外生命值<>，持续 <#e8a800ff>${phDur}秒<>，并<#60e84dff>回复<>该数值一半的生命值（冷却时间${phCd}秒）。"
$i18n.'zh-hans'.radiant_protoplasm_harness.option = "筑防：<#d94c49ff>生命值降至${rphThreshold}%以下<>时，获得 <#60e84dff>${rphFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rphHpPct}%<> 作为<#60e84dff>额外生命值<>，持续 <#e8a800ff>${rphDur}秒<>，并<#60e84dff>回复<>该数值一半的生命值（冷却时间${rphCd}秒）。"
$i18n.'zh-hans'.terminus.option = "交相：普通攻击会交替获得 <#ffdd8eff>${tArmorPen}% <$armorPenIcon> 穿甲<>或 <#88ccffff>${tMagicPen}% <$magicPenIcon> 法术穿透<>，持续 <#e8a800ff>${tDur}秒<>（每种最多叠加${tStacks}层）。"
$i18n.'zh-hans'.radiant_terminus.option = "交相：普通攻击会交替获得 <#ffdd8eff>${rtArmorPen}% <$armorPenIcon> 穿甲<>或 <#88ccffff>${rtMagicPen}% <$magicPenIcon> 法术穿透<>，持续 <#e8a800ff>${rtDur}秒<>（每种最多叠加${rtStacks}层）。"
$i18n.'zh-hans'.collector.option = "死：对生命值低于 <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>最大生命值<> 的敌方英雄造成伤害时，<#d94c49ff>处决<>目标。"
$i18n.'zh-hans'.radiant_collector.option = "死：对生命值低于 <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#60e84dff>最大生命值<> 的敌方英雄造成伤害时，<#d94c49ff>处决<>目标。"
$i18n.'zh-hans'.heartsteel.option = "铁石心肠：每隔 <#e8a800ff>${hsCd}秒<>，你的下一次普通攻击造成 <#ff9028ff>额外物理伤害<>，数值为 <#ff9028ff>${hsFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${hsHpPct}%<>，并永久获得该伤害 <#60e84dff>${hsBonusHpPct}%<> 作为 <#60e84dff>额外生命值<>。"
$i18n.'zh-hans'.radiant_heartsteel.option = "铁石心肠：每隔 <#e8a800ff>${rhsCd}秒<>，你的下一次普通攻击造成 <#ff9028ff>额外物理伤害<>，数值为 <#ff9028ff>${rhsFlat}<> + 你的 <$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>${rhsHpPct}%<>，并永久获得该伤害 <#60e84dff>${rhsBonusHpPct}%<> 作为 <#60e84dff>额外生命值<>。"
$i18n.'zh-hans'.spear_of_shojin.option = "专注意志：技能命中敌方英雄时获得 <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>攻击力<>，持续 <#e8a800ff>${sosDur}秒<>（最多叠加${sosStacks}层）。"
$i18n.'zh-hans'.radiant_spear_of_shojin.option = "专注意志：技能命中敌方英雄时获得 <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>攻击力<>，持续 <#e8a800ff>${rsosDur}秒<>（最多叠加${rsosStacks}层）。"
$i18n.'zh-hans'.warmogs_armor.option = "狂徒之心：每秒回复你 <#60e84dff>${waHeal}%<> <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>最大生命值<>；若在过去 <#e8a800ff>${waDur}秒<> 内未受到伤害，则获得 <#ffffffff>${waMs}%<> <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>移动速度<>。"
$i18n.'zh-hans'.radiant_warmogs_armor.option = "狂徒之心：每秒回复你 <#60e84dff>${rwaHeal}%<> <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>最大生命值<>；若在过去 <#e8a800ff>${rwaDur}秒<> 内未受到伤害，则获得 <#ffffffff>${rwaMs}%<> <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>移动速度<>。"
$i18n.'zh-hans'.stormrazor.option = "蓄能：移动和造成 <#ff9028ff>物理伤害<> 会产生<#e8a800ff>蓄能<>层数，最多 <#e8a800ff>${srStacks}<> 层。`n`n雷霆：<#e8a800ff>蓄能<>充满时，你的下一次 <#ff9028ff>物理伤害<> 额外造成 <#a974ffff>${srDmg} 点魔法伤害<>，并使你获得 <#ffffffff>${srMs}%<> <$speedIcon> <#ffffffff>移动速度<>，持续 <#e8a800ff>${srDur}秒<>。"
$i18n.'zh-hans'.radiant_stormrazor.option = "蓄能：移动和造成 <#ff9028ff>物理伤害<> 会产生<#e8a800ff>蓄能<>层数，最多 <#e8a800ff>${rsrStacks}<> 层。`n`n雷霆：<#e8a800ff>蓄能<>充满时，你的下一次 <#ff9028ff>物理伤害<> 额外造成 <#a974ffff>${rsrDmg} 点魔法伤害<>，并使你获得 <#ffffffff>${rsrMs}%<> <$speedIcon> <#ffffffff>移动速度<>，持续 <#e8a800ff>${rsrDur}秒<>。"

$i18n.'zh-hans'.black_cleaver.option = "碎裂：对敌方英雄造成<#ff9028ff>物理伤害<>会<#d94c49ff>使其 <$armorIcon> <#ffdd8eff>护甲<> 降低${bcShred}%<>，持续 <#e8a800ff>${bcDur}秒<>（最多叠加${bcStacks}层）。"
$i18n.'zh-hans'.radiant_black_cleaver.option = "碎裂：对敌方英雄造成<#ff9028ff>物理伤害<>会<#d94c49ff>使其 <$armorIcon> <#ffdd8eff>护甲<> 降低${rbcShred}%<>，持续 <#e8a800ff>${rbcDur}秒<>（最多叠加${rbcStacks}层）。"
$i18n.'zh-hans'.bloodletters_curse.option = "腐蚀：对敌方英雄造成<#a974ffff>魔法伤害<>会<#d94c49ff>使其 <$mrIcon> <#88ccffff>魔法抗性<> 降低${blcShred}%<>，持续 <#e8a800ff>${blcDur}秒<>（最多叠加${blcStacks}层）。"
$i18n.'zh-hans'.radiant_bloodletters_curse.option = "腐蚀：对敌方英雄造成<#a974ffff>魔法伤害<>会<#d94c49ff>使其 <$mrIcon> <#88ccffff>魔法抗性<> 降低${rblcShred}%<>，持续 <#e8a800ff>${rblcDur}秒<>（最多叠加${rblcStacks}层）。"
$i18n.'zh-hans'.sundered_sky.option = "光盾打击：你对敌方英雄造成的下一次 <#ff9028ff>物理伤害<> 会 <$critIcon> <#d45656ff>暴击<>，造成 <#e8a800ff>${ssDamage}% 额外伤害<>，并<#60e84dff>为你回复<> <#60e84dff>${ssFlatHeal}<> + 你<#60e84dff>已损失生命值<> 的 <#60e84dff>${ssPercentHeal}%<>（每个目标冷却 <#e8a800ff>${ssOnHitCD}秒<>）。"
$i18n.'zh-hans'.radiant_sundered_sky.option = "光盾打击：你对敌方英雄造成的下一次 <#ff9028ff>物理伤害<> 会 <$critIcon> <#d45656ff>暴击<>，造成 <#e8a800ff>${rssDamage}% 额外伤害<>，并<#60e84dff>为你回复<> <#60e84dff>${rssFlatHeal}<> + 你<#60e84dff>已损失生命值<> 的 <#60e84dff>${rssPercentHeal}%<>（每个目标冷却 <#e8a800ff>${rssOnHitCD}秒<>）。"
$i18n.'zh-hans'.echoes_of_helia.option = "灵魂虹吸：将你造成伤害的 <#e8a800ff>${eohConversion}%<> 储存为 <#92dc7bff>灵魂充能<>，最多 <$levelIcon> <#d8c9b3ff>${eohMinCap}<> - <#d8c9b3ff>${eohMaxCap}<>（随<#d8c9b3ff>等级<>提升）。技能命中敌方英雄时，消耗全部 <#92dc7bff>灵魂充能<>，为你<#60e84dff>最近的友军回复<>等量的生命值。"
$i18n.'zh-hans'.radiant_echoes_of_helia.option = "灵魂虹吸：将你造成伤害的 <#e8a800ff>${reohConversion}%<> 储存为 <#92dc7bff>灵魂充能<>，最多 <$levelIcon> <#d8c9b3ff>${reohMinCap}<> - <#d8c9b3ff>${reohMaxCap}<>（随<#d8c9b3ff>等级<>提升）。技能命中敌方英雄时，消耗全部 <#92dc7bff>灵魂充能<>，为你<#60e84dff>最近的友军回复<>等量的生命值。"

$i18n.'zh-hans'.sheen.option = "咒刃：技能命中敌方英雄后，你的下一次普通攻击会造成 <#d8c9b3ff>${sheenMin}<> - <#d8c9b3ff>${sheenMax}<>（基于<$levelIcon> <#d8c9b3ff>等级<>）的<#ff9028ff>额外物理伤害<>（冷却 <#e8a800ff>${sheenCd}秒<>）。"

$tfTemplateZh = "咒刃：技能命中敌方英雄后，你的下一次普通攻击会造成相当于 <#ff9028ff>{0}<> + <$adIcon> <#ff9028ff>攻击力<>的 <#ff9028ff>{1}%<> 的<#ff9028ff>额外物理伤害<>（冷却 <#e8a800ff>{2}秒<>）。"
$i18n.'zh-hans'.trinity_force.option = $tfTemplateZh -f $tfFlat, $tfAdPct, $tfCd
$i18n.'zh-hans'.radiant_trinity_force.option = $tfTemplateZh -f $rtfFlat, $rtfAdPct, $rtfCd

$dndTemplateZh = "咒刃：技能命中敌方英雄后，你的下一次普通攻击会造成相当于 <#a974ffff>{0}<> + <$apIcon> <#a974ffff>法术强度<>的 <#a974ffff>{1}%<> 的<#a974ffff>额外魔法伤害<>，并<#60e84dff>为你回复<>相当于<$apIcon> <#a974ffff>法术强度<>的 <#a974ffff>{2}%<> 与<$hpIcon> <#60e84dff>最大生命值<>的 <#60e84dff>{3}%<> 的生命值（冷却 <#e8a800ff>{4}秒<>）。"
$i18n.'zh-hans'.dusk_and_dawn.option = $dndTemplateZh -f $dndFlat, $dndApPct, $dndApHeal, $dndHpHeal, $dndCd
$i18n.'zh-hans'.radiant_dusk_and_dawn.option = $dndTemplateZh -f $rdndFlat, $rdndApPct, $rdndApHeal, $rdndHpHeal, $rdndCd

$bsTemplateZh = "咒刃：技能命中敌方英雄后，你的下一次普通攻击会造成 <#d8c9b3ff>{0}<> - <#d8c9b3ff>{1}<>（基于<$levelIcon> <#d8c9b3ff>等级<>）的<#a974ffff>额外魔法伤害<>（冷却 <#e8a800ff>{2}秒<>）。如果目标是英雄，使其<#d94c49ff>受到的伤害<>提高 <#d94c49ff>{3}%<>，持续 <#e8a800ff>{4}秒<>。"
$i18n.'zh-hans'.bloodsong.option = $bsTemplateZh -f $bsMin, $bsMax, $bsCd, $bsAmp, $bsDur
$i18n.'zh-hans'.radiant_bloodsong.option = $bsTemplateZh -f $rbsMin, $rbsMax, $rbsCd, $rbsAmp, $rbsDur

$lethZh = "穿甲：对敌人造成伤害时无视 <#ffdd8eff>{0} 点<$armorIcon> 护甲<>。"
$i18n.'zh-hans'.serrated_dirk.option = $lethZh -f $sdLeth
$hubZh = "$lethZh`n`n威望：参与击杀敌方英雄时生成一层永久印记，并获得 <#ff9028ff>{1}<>（每层+{2}）<#ff9028ff>额外<$adIcon> 攻击力<>，持续 <#e8a800ff>{3}秒<>。"
$i18n.'zh-hans'.hubris.option = $hubZh -f $hubLeth, $hubBase, $hubStack, $hubDur
$i18n.'zh-hans'.radiant_hubris.option = $hubZh -f $rhubLeth, $rhubBase, $rhubStack, $rhubDur
$bbZh = "$lethZh`n`n破坏：参与击杀敌方英雄时，获得 <#92dc7bff>破坏<> 效果，持续 <#e8a800ff>{3}秒<>，强化你的下一次对防御塔的攻击，造成 <#ff9028ff>{1}<> + <$adIcon> <#ff9028ff>攻击力<>的 <#ff9028ff>{2}%<> 作为<#ff9028ff>额外物理伤害<>。"
$i18n.'zh-hans'.bastionbreaker.option = $bbZh -f $bbLeth, $bbFlat, $bbPct, $bbDur
$i18n.'zh-hans'.radiant_bastionbreaker.option = $bbZh -f $rbbLeth, $rbbFlat, $rbbPct, $rbbDur
$spfZh = "$lethZh`n`n破盾者：对拥有护盾的敌方英雄造成伤害时，额外造成 <#ff9028ff>{1}<> + <$adIcon> <#ff9028ff>攻击力<>的 <#ff9028ff>{2}%<> 作为<#ff9028ff>额外物理伤害<>。"
$i18n.'zh-hans'.serpents_fang.option = $spfZh -f $spfLeth, $spfFlat, $spfPct
$i18n.'zh-hans'.radiant_serpents_fang.option = $spfZh -f $rspfLeth, $rspfFlat, $rspfPct

Write-Host "Done."
Write-Host "Updating Portuguese (Brazil) text."

$i18n.'pt-BR'.executioners_calling.option = "Ferimentos Graves: Causar <#ff9028ff>dano físico<> a um campeão inimigo <#d94c49ff>reduz a cura dele em ${execHeal}%<> por <#e8a800ff>${execDur} segundos<>."
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
$i18n.'pt-BR'.shadowflame.option = "Floregris: Seu <#a974ffff>dano mágico<> é <#e8a800ff>20% mais forte<> contra inimigos <#d94c49ff>abaixo de ${sfThreshold}% da Vida Máxima<>."
$i18n.'pt-BR'.radiant_shadowflame.option = "Floregris: Seu <#a974ffff>dano mágico<> é <#e8a800ff>20% mais forte<> contra inimigos <#d94c49ff>abaixo de ${rsfThreshold}% da Vida Máxima<>."
$i18n.'pt-BR'.yun_tal_wildarrows.option = "Praticar e Matar: Causar <#ff9028ff>dano físico<> concede <#e8a800ff>${ytCrit}%<> de <$critIcon> <#e8a800ff>Chance de Acerto Crítico<> permanentemente, até <#e8a800ff>${ytMaxCrit}%<>.`n`nAgitação: Ao atacar, ganha <#ceff99ff>${ytFlurryAS}%<> de <$asIcon> <#ceff99ff>Velocidade de Ataque<> por <#e8a800ff>${ytDur} segundos<> (recarga de ${ytCd} segundos)."
$i18n.'pt-BR'.radiant_yun_tal_wildarrows.option = "Praticar e Matar: Causar <#ff9028ff>dano físico<> concede <#e8a800ff>${rytCrit}%<> de <$critIcon> <#e8a800ff>Chance de Acerto Crítico<> permanentemente, até <#e8a800ff>${rytMaxCrit}%<>.`n`nAgitação: Ao atacar, ganha <#ceff99ff>${rytFlurryAS}%<> de <$asIcon> <#ceff99ff>Velocidade de Ataque<> por <#e8a800ff>${rytDur} segundos<> (recarga de ${rytCd} segundos)."
$i18n.'pt-BR'.mortal_reminder.option = "Ferimentos Graves: Causar <#ff9028ff>dano físico<> a um campeão inimigo <#d94c49ff>reduz a cura dele em ${mrHeal}%<> por <#e8a800ff>${mrDur} segundos<>."
$i18n.'pt-BR'.radiant_mortal_reminder.option = "Ferimentos Graves: Causar <#ff9028ff>dano físico<> a um campeão inimigo <#d94c49ff>reduz a cura dele em ${rmrHeal}%<> por <#e8a800ff>${rmrDur} segundos<>."
$i18n.'pt-BR'.jaksho_the_protean.option = "Resiliência: Receber dano de um campeão inimigo concede <#ffdd8eff>${jakDefMult}% <$armorIcon> de Armadura<> e <#88ccffff>${jakMrMult}% <$mrIcon> de Resistência Mágica<> por <#e8a800ff>${jakDur} segundos<> (acumula ${jakStacks}x)."
$i18n.'pt-BR'.radiant_jaksho_the_protean.option = "Resiliência: Receber dano de um campeão inimigo concede <#ffdd8eff>${rjakDefMult}% <$armorIcon> de Armadura<> e <#88ccffff>${rjakMrMult}% <$mrIcon> de Resistência Mágica<> por <#e8a800ff>${rjakDur} segundos<> (acumula ${rjakStacks}x)."
$i18n.'pt-BR'.frozen_mallet.option = "Congelante: Seus ataques aplicam <#d94c49ff>${fmSlow}% de lentidão<> por <#e8a800ff>${fmDur} segundos<>."
$i18n.'pt-BR'.radiant_frozen_mallet.option = "Congelante: Seus ataques causam <#ff9028ff>dano físico<> igual a <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> e aplicam <#d94c49ff>${rfmSlow}% de lentidão<> por <#e8a800ff>${rfmDur} segundos<>."
$i18n.'pt-BR'.rylais_crystal_scepter.option = "Gélido: Causar dano de habilidade aplica <#d94c49ff>${rcsSlow}% de lentidão<> nas unidades atingidas por <#e8a800ff>${rcsDur} segundos<>."
$i18n.'pt-BR'.radiant_rylais_crystal_scepter.option = "Gélido: Causar dano de habilidade aplica <#d94c49ff>${rrcsSlow}% de lentidão<> nas unidades atingidas por <#e8a800ff>${rrcsDur} segundos<>."
$i18n.'pt-BR'.experimental_hexplate.option = "Hexcarregado: Recebe <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>Redução de Tempo de Recarga<> na sua habilidade ultimate."
$i18n.'pt-BR'.radiant_experimental_hexplate.option = "Hexcarregado: Recebe <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>Redução de Tempo de Recarga<> na sua habilidade ultimate."
$i18n.'pt-BR'.guinsoos_rageblade.option = "Ira: Ataques causam <#a974ffff>${gbDmg} de dano mágico adicional<>.`n`nFervendo: Ataques concedem <#ceff99ff>${gbSpeed}%<> de <$asIcon> <#ceff99ff>Velocidade de Ataque<> por <#e8a800ff>${gbDur} segundos<> (acumula ${gbStacks}x)."
$i18n.'pt-BR'.radiant_guinsoos_rageblade.option = "Ira: Ataques causam <#a974ffff>${rgbDmg} de dano mágico adicional<>.`n`nFervendo: Ataques concedem <#ceff99ff>${rgbSpeed}%<> de <$asIcon> <#ceff99ff>Velocidade de Ataque<> por <#e8a800ff>${rgbDur} segundos<> (acumula ${rgbStacks}x)."
$i18n.'pt-BR'.wits_end.option = "Duelo: Ataques causam <#a974ffff>${weDmg} de dano mágico adicional<>."
$i18n.'pt-BR'.radiant_wits_end.option = "Duelo: Ataques causam <#a974ffff>${rweDmg} de dano mágico adicional<>."
$i18n.'pt-BR'.kraken_slayer.option = "Derrubar: A cada terceiro ataque básico, causa <#ff9028ff>${ksDmg} de dano físico bônus<>, aumentado em até <#ff9028ff>${ksBonus}%<> com base na <#60e84dff>vida perdida<> do alvo (bônus máximo com o alvo a <#60e84dff>${ksThresh}%<> de vida)."
$i18n.'pt-BR'.radiant_kraken_slayer.option = "Derrubar: A cada terceiro ataque básico, causa <#ff9028ff>${rksDmg} de dano físico bônus<>, aumentado em até <#ff9028ff>${rksBonus}%<> com base na <#60e84dff>vida perdida<> do alvo (bônus máximo com o alvo a <#60e84dff>${rksThresh}%<> de vida)."
$i18n.'pt-BR'.atmas_reckoning.option = "Monumental: Ganha <#e8a800ff>${arCrit}%<> de <$critIcon> <#e8a800ff>Chance de Acerto Crítico<> a cada <#60e84dff>${arHp}<> de <$hpIcon> <#60e84dff>Vida Máxima<>, até no máximo <#e8a800ff>${arCap}%<>."
$i18n.'pt-BR'.radiant_atmas_reckoning.option = "Monumental: Ganha <#e8a800ff>${rarCrit}%<> de <$critIcon> <#e8a800ff>Chance de Acerto Crítico<> a cada <#60e84dff>${rarHp}<> de <$hpIcon> <#60e84dff>Vida Máxima<>, até no máximo <#e8a800ff>${rarCap}%<>."
$i18n.'pt-BR'.lord_dominiks_regards.option = "Mata-Gigantes: Cause <#ff9028ff>${ldrPct}% de dano bônus<> a cada <#60e84dff>${ldrHp}<> de <$hpIcon> <#60e84dff>Vida Máxima<> do alvo, até <#ff9028ff>${ldrMax}%<>."
$i18n.'pt-BR'.radiant_lord_dominiks_regards.option = "Mata-Gigantes: Cause <#ff9028ff>${rldrPct}% de dano bônus<> a cada <#60e84dff>${rldrHp}<> de <$hpIcon> <#60e84dff>Vida Máxima<> do alvo, até <#ff9028ff>${rldrMax}%<>."
$i18n.'pt-BR'.blackfire_torch.option = "Nefastidão: Suas mágias te dão <#a974ffff>${bftPower}<> <$apIcon> de <#a974ffff>Poder de Habilidade<> por <#e8a800ff>${bftDur} segundos<> (acumula ${bftStacks}x)."
$i18n.'pt-BR'.radiant_blackfire_torch.option = "Nefastidão: Suas mágias te dão <#a974ffff>${rbftPower}<> <$apIcon> de <#a974ffff>Poder de Habilidade<> por <#e8a800ff>${rbftDur} segundos<> (acumula ${rbftStacks}x)."
$i18n.'pt-BR'.blade_of_the_ruined_king.option = "Gume da Névoa: Ataques causam <#d94c49ff>${borkPct}% da Vida Atual do alvo<> como <#ff9028ff>dano físico<> adicional. (Máximo de <#ff9028ff>${borkCap}<> contra tropas e monstros.)"
$i18n.'pt-BR'.radiant_blade_of_the_ruined_king.option = "Gume da Névoa: Ataques causam <#d94c49ff>${rborkPct}% da Vida Atual do alvo<> como <#ff9028ff>dano físico<> adicional. (Máximo de <#ff9028ff>${rborkCap}<> contra tropas e monstros.)"
$i18n.'pt-BR'.deathblade.option = "Apex: Aumenta seu <$adIcon> <#ff9028ff>Dano de Ataque<> em <#ff9028ff>${dbMult}%<>."
$i18n.'pt-BR'.radiant_deathblade.option = "Apex: Aumenta seu <$adIcon> <#ff9028ff>Dano de Ataque<> em <#ff9028ff>${rdbMult}%<>."
$i18n.'pt-BR'.deaths_dance.option = "Ignorar a Dor: <#e8a800ff>${ddDelay}%<> do dano sofrido é armazenado e causado de volta a você ao longo do tempo como <#d94c49ff>dano físico<> (até <#60e84dff>${ddBurnCap}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por segundo).`n`nDesafiar: Participar de um abate de um campeão inimigo remove o dano armazenado restante e <#60e84dff>cura você<> em <#60e84dff>${ddFlatHeal}<> + <#60e84dff>${ddHeal}%<> da sua <#60e84dff>vida perdida<>."
$i18n.'pt-BR'.radiant_deaths_dance.option = "Ignorar a Dor: <#e8a800ff>${rddDelay}%<> do dano sofrido é armazenado e causado de volta a você ao longo do tempo como <#d94c49ff>dano físico<> (até <#60e84dff>${rddBurnCap}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por segundo).`n`nDesafiar: Participar de um abate de um campeão inimigo remove o dano armazenado restante e <#60e84dff>cura você<> em <#60e84dff>${rddFlatHeal}<> + <#60e84dff>${rddHeal}%<> da sua <#60e84dff>vida perdida<>."
$i18n.'pt-BR'.rabadons_deathcap.option = "Apogeu: Aumenta o <$apIcon> <#a974ffff>Poder de Habilidade<> total em <#a974ffff>${rabMult}%<>."
$i18n.'pt-BR'.radiant_rabadons_deathcap.option = "Apogeu: Aumenta o <$apIcon> <#a974ffff>Poder de Habilidade<> total em <#a974ffff>${radRabMult}%<>"

$mbIllusionPt = "Ilusão: Ganha <#d48294ff>{0}<> <$forceIcon> de <#d48294ff>Força Adaptativa<>. Cada <$forceIcon> <#d48294ff>Força Adaptativa<> garante <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Dano de Ataque<> ou <#a974ffff>1<> <$apIcon> <#a974ffff>Poder de Habilidade<>, dependendo de qual é maior.`n`nBorrão: Abates concedem <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>Velocidade de Movimento<> por <#e8a800ff>{2} segundos<>."
$i18n.'pt-BR'.mirage_blade.option = $mbIllusionPt -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.'pt-BR'.radiant_mirage_blade.option = $mbIllusionPt -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$dtsPiercePt = "Perfuração: Ganha <#d48294ff>{0}<> <$forceIcon> de <#d48294ff>Força Adaptativa<>. Cada <$forceIcon> <#d48294ff>Força Adaptativa<> garante <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Dano de Ataque<> ou <#a974ffff>1<> <$apIcon> <#a974ffff>Poder de Habilidade<>, dependendo de qual é maior.`n`nPonto Ideal: Causa até <#e8a800ff>{1}% de dano bônus<> a campeões inimigos com base na distância (efeito máximo em <#ff86c2ff>{2} <$rangeIcon> alcance<>)."
$i18n.'pt-BR'.diamond_tipped_spear.option = $dtsPiercePt -f $dtsForce, $dtsPct, $dtsDist
$i18n.'pt-BR'.radiant_diamond_tipped_spear.option = $dtsPiercePt -f $rdtsForce, $rdtsPct, $rdtsDist
$zekAuraPt = "Aura: Concede <#d48294ff>{0}<> <$forceIcon> de <#d48294ff>Força Adaptativa<> e <#b7462dff>{2}%<> <$vampIcon> <#b7462dff>roubo de vida<> a todos os campeões aliados dentro de <#ff86c2ff>{1} <$rangeIcon> alcance<>."
$i18n.'pt-BR'.zekes_herald.option = $zekAuraPt -f $zekForce, $zekDist, $zekVamp
$i18n.'pt-BR'.radiant_zekes_herald.option = $zekAuraPt -f $rzekForce, $rzekDist, $rzekVamp
$ssBullseyePt = "Na Mosca: Causar dano a um campeão inimigo causa <#a974ffff>{0} de dano mágico bônus<> (recarga de <#e8a800ff>{1} segundos<>)."
$i18n.'pt-BR'.scouts_slingshot.option = $ssBullseyePt -f $ssDmg, $ssCd
$litSufferingPt = "Sofrimento: Causar dano de habilidade queima os inimigos, fazendo-os sofrer <#d94c49ff>{0}% da Vida Máxima deles<> como <#a974ffff>dano mágico<> ao longo de <#e8a800ff>{1} segundos<>. Máximo de <#a974ffff>{2} de dano mágico<> por tique contra tropas e monstros."
$i18n.'pt-BR'.liandrys_torment.option = $litSufferingPt -f $litHp, $litDur, $litCap
$i18n.'pt-BR'.radiant_liandrys_torment.option = $litSufferingPt -f $rlitHp, $rlitDur, $rlitCap
$i18n.'pt-BR'.spirit_visage.option = "Vitalidade: <#60e84dff>Curas recebidas<> são aumentadas em <#60e84dff>${svHeal}%<>."
$i18n.'pt-BR'.radiant_spirit_visage.option = "Vitalidade: <#60e84dff>Curas recebidas<> são aumentadas em <#60e84dff>${rsvHeal}%<>."
$i18n.'pt-BR'.unending_despair.option = "Angústia: Suas mágias te curam em <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>."
$i18n.'pt-BR'.radiant_unending_despair.option = "Angústia: Suas mágias te curam em <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>."
$i18n.'pt-BR'.protoplasm_harness.option = "Fortificação: Ao receber dano suficiente para reduzir sua <#d94c49ff>Vida a menos de ${phThreshold}%<> você, recebe <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por <#e8a800ff>${phDur} segundos<>, depois <#60e84dff>cura<> por metade desse valor (${phCd}s)."
$i18n.'pt-BR'.radiant_protoplasm_harness.option = "Fortificação: Ao receber dano suficiente para reduzir sua <#d94c49ff>Vida a menos de ${rphThreshold}%<> você, recebe <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> por <#e8a800ff>${rphDur} segundos<>, depois <#60e84dff>cura<> por metade desse valor (${rphCd}s)."
$i18n.'pt-BR'.terminus.option = "Justaposição: Ataques alternam para conceder entre <#ffdd8eff>${tArmorPen}% <$armorPenIcon> de Penetração de Armadura<> ou <#88ccffff>${tMagicPen}% <$magicPenIcon> Penetração Mágica<> por <#e8a800ff>${tDur} segundos<>. (acumula ${tStacks}x cada)."
$i18n.'pt-BR'.radiant_terminus.option = "Justaposição: Ataques alternam para conceder entre <#ffdd8eff>${rtArmorPen}% <$armorPenIcon> de Penetração de Armadura<> ou <#88ccffff>${rtMagicPen}% <$magicPenIcon> Penetração Mágica<> por <#e8a800ff>${rtDur} segundos<>. (acumula ${rtStacks}x cada)."
$i18n.'pt-BR'.collector.option = "Morte: Causar dano a campeões inimigos com menos de <#60e84dff>${colThreshold}%<> <$hpIcon> <#60e84dff>Vida Máxima<> os <#d94c49ff>executa<>."
$i18n.'pt-BR'.radiant_collector.option = "Morte: Causar dano a campeões inimigos com menos de <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#60e84dff>Vida Máxima<> os <#d94c49ff>executa<>."
$i18n.'pt-BR'.heartsteel.option = "Coração Forjado: A cada <#e8a800ff>${hsCd} segundos<>, seu próximo ataque causa <#ff9028ff>dano físico<> adicional igual a <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>, concedendo permanentemente <#60e84dff>${hsBonusHpPct}%<> desse dano como <#60e84dff>vida bônus<>."
$i18n.'pt-BR'.radiant_heartsteel.option = "Coração Forjado: A cada <#e8a800ff>${rhsCd} segundos<>, seu próximo ataque causa <#ff9028ff>dano físico<> adicional igual a <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<>, concedendo permanentemente <#60e84dff>${rhsBonusHpPct}%<> desse dano como <#60e84dff>vida bônus<>."
$i18n.'pt-BR'.spear_of_shojin.option = "Vontade Focada: Acertar habilidades em campeões inimigos concede <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>Dano de Ataque<> por <#e8a800ff>${sosDur} segundos<> (acumula ${sosStacks}x)."
$i18n.'pt-BR'.radiant_spear_of_shojin.option = "Vontade Focada: Acertar habilidades em campeões inimigos concede <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>Dano de Ataque<> por <#e8a800ff>${rsosDur} segundos<> (acumula ${rsosStacks}x)."
$i18n.'pt-BR'.warmogs_armor.option = "Coração de Warmog: Regenera <#60e84dff>${waHeal}%<> da sua <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>Vida Máxima<> por segundo e concede <#ffffffff>${waMs}%<> de <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>Velocidade de Movimento<> se você não sofrer dano nos últimos <#e8a800ff>${waDur} segundos<>."
$i18n.'pt-BR'.radiant_warmogs_armor.option = "Coração de Warmog: Regenera <#60e84dff>${rwaHeal}%<> da sua <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>Vida Máxima<> por segundo e concede <#ffffffff>${rwaMs}%<> de <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>Velocidade de Movimento<> se você não sofrer dano nos últimos <#e8a800ff>${rwaDur} segundos<>."
$i18n.'pt-BR'.stormrazor.option = "Energizado: Mover-se e causar <#ff9028ff>dano físico<> gera acúmulos de <#e8a800ff>Energia<>, até <#e8a800ff>${srStacks}<>.`n`nRaio: Quando totalmente <#e8a800ff>Energizado<>, sua próxima instância de <#ff9028ff>dano físico<> causa <#a974ffff>${srDmg} de dano mágico bônus<> e concede <#ffffffff>${srMs}%<> de <$speedIcon> <#ffffffff>Velocidade de Movimento<> por <#e8a800ff>${srDur} segundos<>."
$i18n.'pt-BR'.radiant_stormrazor.option = "Energizado: Mover-se e causar <#ff9028ff>dano físico<> gera acúmulos de <#e8a800ff>Energia<>, até <#e8a800ff>${rsrStacks}<>.`n`nRaio: Quando totalmente <#e8a800ff>Energizado<>, sua próxima instância de <#ff9028ff>dano físico<> causa <#a974ffff>${rsrDmg} de dano mágico bônus<> e concede <#ffffffff>${rsrMs}%<> de <$speedIcon> <#ffffffff>Velocidade de Movimento<> por <#e8a800ff>${rsrDur} segundos<>."

$i18n.'pt-BR'.black_cleaver.option = "Retalhar: Causar <#ff9028ff>dano físico<> a campeões inimigos <#d94c49ff>reduz a <$armorIcon> <#ffdd8eff>Armadura<> deles em ${bcShred}%<> por <#e8a800ff>${bcDur} segundos<> (acumula ${bcStacks}x)."
$i18n.'pt-BR'.radiant_black_cleaver.option = "Retalhar: Causar <#ff9028ff>dano físico<> a campeões inimigos <#d94c49ff>reduz a <$armorIcon> <#ffdd8eff>Armadura<> deles em ${rbcShred}%<> por <#e8a800ff>${rbcDur} segundos<> (acumula ${rbcStacks}x)."
$i18n.'pt-BR'.bloodletters_curse.option = "Decaimento: Causar <#a974ffff>dano mágico<> a campeões inimigos <#d94c49ff>reduz a <$mrIcon> <#88ccffff>Resistência Mágica<> deles em ${blcShred}%<> por <#e8a800ff>${blcDur} segundos<> (acumula ${blcStacks}x)."
$i18n.'pt-BR'.radiant_bloodletters_curse.option = "Decaimento: Causar <#a974ffff>dano mágico<> a campeões inimigos <#d94c49ff>reduz a <$mrIcon> <#88ccffff>Resistência Mágica<> deles em ${rblcShred}%<> por <#e8a800ff>${rblcDur} segundos<> (acumula ${rblcStacks}x)."
$i18n.'pt-BR'.sundered_sky.option = "Golpe do Escudo de Luz: Sua próxima instância de <#ff9028ff>dano físico<> contra um campeão inimigo <$critIcon> <#d45656ff>causa acerto crítico<>, causando <#e8a800ff>${ssDamage}% de dano bônus<>, e <#60e84dff>cura você<> em <#60e84dff>${ssFlatHeal}<> + <#60e84dff>${ssPercentHeal}%<> da sua <#60e84dff>vida perdida<> (recarga de <#e8a800ff>${ssOnHitCD} segundos<> por alvo)."
$i18n.'pt-BR'.radiant_sundered_sky.option = "Golpe do Escudo de Luz: Sua próxima instância de <#ff9028ff>dano físico<> contra um campeão inimigo <$critIcon> <#d45656ff>causa acerto crítico<>, causando <#e8a800ff>${rssDamage}% de dano bônus<>, e <#60e84dff>cura você<> em <#60e84dff>${rssFlatHeal}<> + <#60e84dff>${rssPercentHeal}%<> da sua <#60e84dff>vida perdida<> (recarga de <#e8a800ff>${rssOnHitCD} segundos<> por alvo)."
$i18n.'pt-BR'.echoes_of_helia.option = "Sifão de Almas: Armazena <#e8a800ff>${eohConversion}%<> do dano causado como <#92dc7bff>Cargas de Alma<>, até <$levelIcon> <#d8c9b3ff>${eohMinCap}<> - <#d8c9b3ff>${eohMaxCap}<> (escalando com o <#d8c9b3ff>nível<>). Acertar uma Habilidade em um campeão inimigo consome todas as <#92dc7bff>Cargas de Alma<>, <#60e84dff>curando o aliado mais próximo<> na quantidade consumida."
$i18n.'pt-BR'.radiant_echoes_of_helia.option = "Sifão de Almas: Armazena <#e8a800ff>${reohConversion}%<> do dano causado como <#92dc7bff>Cargas de Alma<>, até <$levelIcon> <#d8c9b3ff>${reohMinCap}<> - <#d8c9b3ff>${reohMaxCap}<> (escalando com o <#d8c9b3ff>nível<>). Acertar uma Habilidade em um campeão inimigo consome todas as <#92dc7bff>Cargas de Alma<>, <#60e84dff>curando o aliado mais próximo<> na quantidade consumida."

$i18n.'pt-BR'.sheen.option = "Lâmina Arcana: Acertar uma Habilidade em um campeão inimigo faz seu próximo ataque causar <#d8c9b3ff>${sheenMin}<> - <#d8c9b3ff>${sheenMax}<> (com base no <$levelIcon> <#d8c9b3ff>nível<>) como <#ff9028ff>dano físico bônus<> (recarga de <#e8a800ff>${sheenCd} segundos<>)."

$tfTemplatePt = "Lâmina Arcana: Acertar uma Habilidade em um campeão inimigo faz seu próximo ataque causar <#ff9028ff>{0}<> + <#ff9028ff>{1}%<> do seu <$adIcon> <#ff9028ff>Dano de Ataque<> como <#ff9028ff>dano físico bônus<> (recarga de <#e8a800ff>{2} segundos<>)."
$i18n.'pt-BR'.trinity_force.option = $tfTemplatePt -f $tfFlat, $tfAdPct, $tfCd
$i18n.'pt-BR'.radiant_trinity_force.option = $tfTemplatePt -f $rtfFlat, $rtfAdPct, $rtfCd

$dndTemplatePt = "Lâmina Arcana: Acertar uma Habilidade em um campeão inimigo faz seu próximo ataque causar <#a974ffff>{0}<> + <#a974ffff>{1}%<> do seu <$apIcon> <#a974ffff>Poder de Habilidade<> como <#a974ffff>dano mágico bônus<> e <#60e84dff>curar você<> em <#a974ffff>{2}%<> do seu <$apIcon> <#a974ffff>Poder de Habilidade<> e <#60e84dff>{3}%<> da sua <$hpIcon> <#60e84dff>Vida Máxima<> (recarga de <#e8a800ff>{4} segundos<>)."
$i18n.'pt-BR'.dusk_and_dawn.option = $dndTemplatePt -f $dndFlat, $dndApPct, $dndApHeal, $dndHpHeal, $dndCd
$i18n.'pt-BR'.radiant_dusk_and_dawn.option = $dndTemplatePt -f $rdndFlat, $rdndApPct, $rdndApHeal, $rdndHpHeal, $rdndCd

$bsTemplatePt = "Lâmina Arcana: Acertar uma Habilidade em um campeão inimigo faz seu próximo ataque causar <#d8c9b3ff>{0}<> - <#d8c9b3ff>{1}<> (com base no <$levelIcon> <#d8c9b3ff>nível<>) como <#a974ffff>dano mágico bônus<> (recarga de <#e8a800ff>{2} segundos<>). Se o alvo for um campeão, aumenta o <#d94c49ff>dano que ele recebe<> em <#d94c49ff>{3}%<> por <#e8a800ff>{4} segundos<>."
$i18n.'pt-BR'.bloodsong.option = $bsTemplatePt -f $bsMin, $bsMax, $bsCd, $bsAmp, $bsDur
$i18n.'pt-BR'.radiant_bloodsong.option = $bsTemplatePt -f $rbsMin, $rbsMax, $rbsCd, $rbsAmp, $rbsDur

$lethPt = "Letalidade: Ignora <#ffdd8eff>{0} de <$armorIcon> Armadura<> ao causar dano a inimigos."
$i18n.'pt-BR'.serrated_dirk.option = $lethPt -f $sdLeth
$hubPt = "$lethPt`n`nEminência: Ao participar do abate de um campeão inimigo, gera um acúmulo permanente e concede <#ff9028ff>{1}<> (+{2} por acúmulo) de <$adIcon> <#ff9028ff>Dano de Ataque bônus<> por <#e8a800ff>{3} segundos<>."
$i18n.'pt-BR'.hubris.option = $hubPt -f $hubLeth, $hubBase, $hubStack, $hubDur
$i18n.'pt-BR'.radiant_hubris.option = $hubPt -f $rhubLeth, $rhubBase, $rhubStack, $rhubDur
$bbPt = "$lethPt`n`nSabotagem: Ao participar do abate de um campeão inimigo, concede <#92dc7bff>Sabotagem<> por <#e8a800ff>{3} segundos<>, potencializando seu próximo Ataque contra uma torre para causar <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> do seu <$adIcon> <#ff9028ff>Dano de Ataque<> como <#ff9028ff>dano físico bônus<>."
$i18n.'pt-BR'.bastionbreaker.option = $bbPt -f $bbLeth, $bbFlat, $bbPct, $bbDur
$i18n.'pt-BR'.radiant_bastionbreaker.option = $bbPt -f $rbbLeth, $rbbFlat, $rbbPct, $rbbDur
$spfPt = "$lethPt`n`nCeifador de Escudos: Causar dano a um campeão inimigo com escudo causa <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> do seu <$adIcon> <#ff9028ff>Dano de Ataque<> como <#ff9028ff>dano físico bônus<>."
$i18n.'pt-BR'.serpents_fang.option = $spfPt -f $spfLeth, $spfFlat, $spfPct
$i18n.'pt-BR'.radiant_serpents_fang.option = $spfPt -f $rspfLeth, $rspfFlat, $rspfPct

Write-Host "Done."
Write-Host "Updating Russian text."

$i18n.ru.executioners_calling.option = "Тяжёлые раны: Нанесение <#ff9028ff>физического урона<> вражескому чемпиону <#d94c49ff>снижает его лечение на ${execHeal}%<> на <#e8a800ff>${execDur} секунды<>."
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
$i18n.ru.shadowflame.option = "Огненный цветок: Ваш <#a974ffff>магический урон<> <#e8a800ff>на 20% сильнее<> против врагов, чьё <#d94c49ff>здоровье ниже ${sfThreshold}% от максимального<>."
$i18n.ru.radiant_shadowflame.option = "Огненный цветок: Ваш <#a974ffff>магический урон<> <#e8a800ff>на 20% сильнее<> против врагов, чьё <#d94c49ff>здоровье ниже ${rsfThreshold}% от максимального<>."
$i18n.ru.yun_tal_wildarrows.option = "Убийственная практика: Нанесение <#ff9028ff>физического урона<> навсегда даёт <#e8a800ff>${ytCrit}%<> <$critIcon> <#e8a800ff>шанса критического удара<>, вплоть до <#e8a800ff>${ytMaxCrit}%<>.`n`nШквал: При атаке даёт <#ceff99ff>${ytFlurryAS}%<> <$asIcon> <#ceff99ff>скорости атаки<> на <#e8a800ff>${ytDur} секунд<> (перезарядка ${ytCd} секунд)."
$i18n.ru.radiant_yun_tal_wildarrows.option = "Убийственная практика: Нанесение <#ff9028ff>физического урона<> навсегда даёт <#e8a800ff>${rytCrit}%<> <$critIcon> <#e8a800ff>шанса критического удара<>, вплоть до <#e8a800ff>${rytMaxCrit}%<>.`n`nШквал: При атаке даёт <#ceff99ff>${rytFlurryAS}%<> <$asIcon> <#ceff99ff>скорости атаки<> на <#e8a800ff>${rytDur} секунд<> (перезарядка ${rytCd} секунд)."
$i18n.ru.mortal_reminder.option = "Тяжёлые раны: Нанесение <#ff9028ff>физического урона<> вражескому чемпиону <#d94c49ff>снижает его лечение на ${mrHeal}%<> на <#e8a800ff>${mrDur} секунды<>."
$i18n.ru.radiant_mortal_reminder.option = "Тяжёлые раны: Нанесение <#ff9028ff>физического урона<> вражескому чемпиону <#d94c49ff>снижает его лечение на ${rmrHeal}%<> на <#e8a800ff>${rmrDur} секунды<>."
$i18n.ru.jaksho_the_protean.option = "Стойкость: Получение урона от вражеского чемпиона даёт <#ffdd8eff>${jakDefMult}% <$armorIcon> брони<> и <#88ccffff>${jakMrMult}% <$mrIcon> сопротивления магии<> на <#e8a800ff>${jakDur} секунды<> (макс. ${jakStacks} стака)."
$i18n.ru.radiant_jaksho_the_protean.option = "Стойкость: Получение урона от вражеского чемпиона даёт <#ffdd8eff>${rjakDefMult}% <$armorIcon> брони<> и <#88ccffff>${rjakMrMult}% <$mrIcon> сопротивления магии<> на <#e8a800ff>${rjakDur} секунды<> (макс. ${rjakStacks} стака)."
$i18n.ru.frozen_mallet.option = "Обледенение: При атаке накладывает <#d94c49ff>замедление на ${fmSlow}%<> на <#e8a800ff>${fmDur} секунды<>."
$i18n.ru.radiant_frozen_mallet.option = "Обледенение: При атаке наносит <#ff9028ff>дополнительный физический урон<> равный <#ff9028ff>${rfmFlat}<> + <#60e84dff>${rfmHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> и накладывает <#d94c49ff>замедление на ${rfmSlow}%<> на <#e8a800ff>${rfmDur} секунды<>."
$i18n.ru.rylais_crystal_scepter.option = "Иней: Нанесение урона умением замедляет пораженные цели на <#d94c49ff>${rcsSlow}%<> на <#e8a800ff>${rcsDur} секунды<>."
$i18n.ru.radiant_rylais_crystal_scepter.option = "Иней: Нанесение урона умением замедляет пораженные цели на <#d94c49ff>${rrcsSlow}%<> на <#e8a800ff>${rrcsDur} секунды<>."
$i18n.ru.experimental_hexplate.option = "Перегрузка: Даёт <#4b7cffff>${hexUltCdr}%<> <$cdrIcon> <#4b7cffff>сокращения времени перезарядки<> вашего ультимейта."
$i18n.ru.radiant_experimental_hexplate.option = "Перегрузка: Даёт <#4b7cffff>${rhexUltCdr}%<> <$cdrIcon> <#4b7cffff>сокращения времени перезарядки<> вашего ультимейта."
$i18n.ru.guinsoos_rageblade.option = "Гнев: Ваши базовые атаки наносят <#a974ffff>${gbDmg} дополнительного магического урона<>.`n`nЯростный Удар: При атаке даёт <#ceff99ff>${gbSpeed}%<> <$asIcon> <#ceff99ff>скорости атаки<> на <#e8a800ff>${gbDur} секунды<> (макс. ${gbStacks} стака)."
$i18n.ru.radiant_guinsoos_rageblade.option = "Гнев: Ваши базовые атаки наносят <#a974ffff>${rgbDmg} дополнительного магического урона<>.`n`nЯростный Удар: При атаке даёт <#ceff99ff>${rgbSpeed}%<> <$asIcon> <#ceff99ff>скорости атаки<> на <#e8a800ff>${rgbDur} секунды<> (макс. ${rgbStacks} стака)."
$i18n.ru.wits_end.option = "Боевой запал: Ваши базовые атаки наносят <#a974ffff>${weDmg} дополнительного магического урона<>."
$i18n.ru.radiant_wits_end.option = "Боевой запал: Ваши базовые атаки наносят <#a974ffff>${rweDmg} дополнительного магического урона<>."
$i18n.ru.kraken_slayer.option = "Тяжелая артиллерия: Каждая третья базовая атака наносит <#ff9028ff>${ksDmg} дополнительного физического урона<>, увеличиваясь вплоть до <#ff9028ff>${ksBonus}%<> в зависимости от <#60e84dff>потерянного здоровья<> цели (максимум при <#60e84dff>${ksThresh}%<> здоровья цели)."
$i18n.ru.radiant_kraken_slayer.option = "Тяжелая артиллерия: Каждая третья базовая атака наносит <#ff9028ff>${rksDmg} дополнительного физического урона<>, увеличиваясь вплоть до <#ff9028ff>${rksBonus}%<> в зависимости от <#60e84dff>потерянного здоровья<> цели (максимум при <#60e84dff>${rksThresh}%<> здоровья цели)."
$i18n.ru.atmas_reckoning.option = "Большие руки: Даёт <#e8a800ff>${arCrit}%<> <$critIcon> <#e8a800ff>шанса критического удара<> за каждые <#60e84dff>${arHp}<> <$hpIcon> <#60e84dff>максимального здоровья<>, вплоть до <#e8a800ff>${arCap}%<>."
$i18n.ru.radiant_atmas_reckoning.option = "Большие руки: Даёт <#e8a800ff>${rarCrit}%<> <$critIcon> <#e8a800ff>шанса критического удара<> за каждые <#60e84dff>${rarHp}<> <$hpIcon> <#60e84dff>максимального здоровья<>, вплоть до <#e8a800ff>${rarCap}%<>."
$i18n.ru.lord_dominiks_regards.option = "Убийца великанов: Наносит <#ff9028ff>${ldrPct}% дополнительного урона<> за каждые <#60e84dff>${ldrHp}<> <$hpIcon> <#60e84dff>максимального здоровья<> цели, вплоть до <#ff9028ff>${ldrMax}%<>."
$i18n.ru.radiant_lord_dominiks_regards.option = "Убийца великанов: Наносит <#ff9028ff>${rldrPct}% дополнительного урона<> за каждые <#60e84dff>${rldrHp}<> <$hpIcon> <#60e84dff>максимального здоровья<> цели, вплоть до <#ff9028ff>${rldrMax}%<>."
$i18n.ru.blackfire_torch.option = "Злодейский: Попадания умениями дают <#a974ffff>${bftPower}<> <$apIcon> <#a974ffff>Силы Умений<> на <#e8a800ff>${bftDur} секунды<> (макс. ${bftStacks} стака)."
$i18n.ru.radiant_blackfire_torch.option = "Злодейский: Попадания умениями дают <#a974ffff>${rbftPower}<> <$apIcon> <#a974ffff>Силы Умений<> на <#e8a800ff>${rbftDur} секунды<> (макс. ${rbftStacks} стака)."
$i18n.ru.blade_of_the_ruined_king.option = "Край тумана: При атаке наносит <#ff9028ff>дополнительный физический урон<> равный <#d94c49ff>${borkPct}% текущего здоровья цели<>. Наносит максимум <#ff9028ff>${borkCap} физического урона<> по миньонам и монстрам."
$i18n.ru.radiant_blade_of_the_ruined_king.option = "Край тумана: При атаке наносит <#ff9028ff>дополнительный физический урон<> равный <#d94c49ff>${rborkPct}% текущего здоровья цели<>. Наносит максимум <#ff9028ff>${rborkCap} физического урона<> по миньонам и монстрам."
$i18n.ru.deathblade.option = "Вершина: Увеличивает вашу общую <$adIcon> <#ff9028ff>Силу Атаки<> на <#ff9028ff>${dbMult}%<>."
$i18n.ru.radiant_deathblade.option = "Вершина: Увеличивает вашу общую <$adIcon> <#ff9028ff>Силу Атаки<> на <#ff9028ff>${rdbMult}%<>."
$i18n.ru.deaths_dance.option = "Игнорирование Боли: <#e8a800ff>${ddDelay}%<> получаемого урона вместо этого наносится с течением времени как <#d94c49ff>физический урон<> (до <#60e84dff>${ddBurnCap}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> в секунду).`n`nВызов: Участие в убийстве вражеского чемпиона снимает оставшийся накопленный урон и <#60e84dff>восстанавливает<> <#60e84dff>${ddFlatHeal}<> + <#60e84dff>${ddHeal}%<> от вашего <#60e84dff>потерянного здоровья<>."
$i18n.ru.radiant_deaths_dance.option = "Игнорирование Боли: <#e8a800ff>${rddDelay}%<> получаемого урона вместо этого наносится с течением времени как <#d94c49ff>физический урон<> (до <#60e84dff>${rddBurnCap}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> в секунду).`n`nВызов: Участие в убийстве вражеского чемпиона снимает оставшийся накопленный урон и <#60e84dff>восстанавливает<> <#60e84dff>${rddFlatHeal}<> + <#60e84dff>${rddHeal}%<> от вашего <#60e84dff>потерянного здоровья<>."
$i18n.ru.rabadons_deathcap.option = "Опус: Увеличивает вашу общую <$apIcon> <#a974ffff>Силу Умений<> на <#a974ffff>${rabMult}%<>."
$i18n.ru.radiant_rabadons_deathcap.option = "Опус: Увеличивает вашу общую <$apIcon> <#a974ffff>Силу Умений<> на <#a974ffff>${radRabMult}%<>."

$mbIllusionRu = "Иллюзия: Даёт <#d48294ff>{0}<> <$forceIcon> <#d48294ff>адаптивной силы<>. Каждая единица <$forceIcon> <#d48294ff>адаптивной силы<> даёт <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Силы атаки<> или <#a974ffff>1<> <$apIcon> <#a974ffff>Силы умений<>, в зависимости от того, что выше.`n`nРазымытие: При убийстве даёт <#ffffffff>{1}%<> <$speedIcon> <#ffffffff>скорости передвижения<> на <#e8a800ff>{2} секунды<>."
$i18n.ru.mirage_blade.option = $mbIllusionRu -f $mbForce, $mbMoveSpeed, $mbDuration
$i18n.ru.radiant_mirage_blade.option = $mbIllusionRu -f $rmbForce, $rmbMoveSpeed, $rmbDuration
$dtsPierceRu = "Пронзание: Даёт <#d48294ff>{0}<> <$forceIcon> <#d48294ff>адаптивной силы<>. Каждая единица <$forceIcon> <#d48294ff>адаптивной силы<> даёт <#ff9028ff>0.6<> <$adIcon> <#ff9028ff>Силы атаки<> или <#a974ffff>1<> <$apIcon> <#a974ffff>Силы умений<>, в зависимости от того, что выше.`n`nИдеальная дистанция: Наносит до <#e8a800ff>{1}% дополнительного урона<> вражеским чемпионам в зависимости от расстояния (максимальный эффект на <#ff86c2ff>{2} <$rangeIcon> дальности<>)."
$i18n.ru.diamond_tipped_spear.option = $dtsPierceRu -f $dtsForce, $dtsPct, $dtsDist
$i18n.ru.radiant_diamond_tipped_spear.option = $dtsPierceRu -f $rdtsForce, $rdtsPct, $rdtsDist
$zekAuraRu = "Аура: Даёт <#d48294ff>{0}<> <$forceIcon> <#d48294ff>адаптивной силы<> и <#b7462dff>{2}%<> <$vampIcon> <#b7462dff>всестороннего вытягивания жизни<> всем союзным чемпионам в пределах <#ff86c2ff>{1} <$rangeIcon> дальности<>."
$i18n.ru.zekes_herald.option = $zekAuraRu -f $zekForce, $zekDist, $zekVamp
$i18n.ru.radiant_zekes_herald.option = $zekAuraRu -f $rzekForce, $rzekDist, $rzekVamp
$ssBullseyeRu = "В яблочко: Нанесение урона вражескому чемпиону наносит <#a974ffff>{0} дополнительного магического урона<> (перезарядка <#e8a800ff>{1} секунд<>)."
$i18n.ru.scouts_slingshot.option = $ssBullseyeRu -f $ssDmg, $ssCd
$litSufferingRu = "Страдание: Нанесение урона умением поджигает врагов, заставляя их получить <#d94c49ff>{0}% их максимального здоровья<> в виде <#a974ffff>магического урона<> в течение <#e8a800ff>{1} секунд<>. Наносит максимум <#a974ffff>{2} магического урона<> за тик по миньонам и монстрам."
$i18n.ru.liandrys_torment.option = $litSufferingRu -f $litHp, $litDur, $litCap
$i18n.ru.radiant_liandrys_torment.option = $litSufferingRu -f $rlitHp, $rlitDur, $rlitCap
$i18n.ru.spirit_visage.option = "Живительная Сила: Увеличивает всё <#60e84dff>получаемое лечение<> на <#60e84dff>${svHeal}%<>."
$i18n.ru.radiant_spirit_visage.option = "Живительная Сила: Увеличивает всё <#60e84dff>получаемое лечение<> на <#60e84dff>${rsvHeal}%<>."
$i18n.ru.unending_despair.option = "Страдание: Попадания умениями восстанавливают вам <#60e84dff>${udFlat}<> + <#60e84dff>${udHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>."
$i18n.ru.radiant_unending_despair.option = "Страдание: Попадания умениями восстанавливают вам <#60e84dff>${rudFlat}<> + <#60e84dff>${rudHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>."
$i18n.ru.protoplasm_harness.option = "Укрепление: При <#d94c49ff>падении здоровья ниже ${phThreshold}%<> даёт <#60e84dff>${phFlat}<> + <#60e84dff>${phHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> как <#60e84dff>дополнительное здоровье<> на <#e8a800ff>${phDur} секунд<> и <#60e84dff>восстанавливает вам<> половину этого количества (перезарядка ${phCd} секунд)."
$i18n.ru.radiant_protoplasm_harness.option = "Укрепление: При <#d94c49ff>падении здоровья ниже ${rphThreshold}%<> даёт <#60e84dff>${rphFlat}<> + <#60e84dff>${rphHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<> как <#60e84dff>дополнительное здоровье<> на <#e8a800ff>${rphDur} секунд<> и <#60e84dff>восстанавливает вам<> половину этого количества (перезарядка ${rphCd} секунд)."
$i18n.ru.terminus.option = "Соприкосновение: При атаке получает либо <#ffdd8eff>${tArmorPen}% <$armorPenIcon> пробивания брони<>, либо <#88ccffff>${tMagicPen}% <$magicPenIcon> пробивания магии<> на <#e8a800ff>${tDur} секунды<>, чередуясь (макс. ${tStacks} стака каждого)."
$i18n.ru.radiant_terminus.option = "Соприкосновение: При атаке получает либо <#ffdd8eff>${rtArmorPen}% <$armorPenIcon> пробивания брони<>, либо <#88ccffff>${rtMagicPen}% <$magicPenIcon> пробивания магии<> на <#e8a800ff>${rtDur} секунды<>, чередуясь (макс. ${rtStacks} стака каждого)."
$i18n.ru.collector.option = "Смерть: Нанесение урона вражеским чемпионам с <#60e84dff>максимальным здоровьем<> ниже <#60e84dff>${colThreshold}%<> <$hpIcon> <#d94c49ff>казнит<> их."
$i18n.ru.radiant_collector.option = "Смерть: Нанесение урона вражеским чемпионам с <#60e84dff>максимальным здоровьем<> ниже <#60e84dff>${rcolThreshold}%<> <$hpIcon> <#d94c49ff>казнит<> их."
$i18n.ru.heartsteel.option = "Железное сердце: Каждые <#e8a800ff>${hsCd} секунд<>, ваша следующая атака наносит <#ff9028ff>дополнительный физический урон<> равный <#ff9028ff>${hsFlat}<> + <#60e84dff>${hsHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>, и даёт <#60e84dff>${hsBonusHpPct}%<> от этого урона как постоянное <#60e84dff>дополнительное здоровье<>."
$i18n.ru.radiant_heartsteel.option = "Железное сердце: Каждые <#e8a800ff>${rhsCd} секунд<>, ваша следующая атака наносит <#ff9028ff>дополнительный физический урон<> равный <#ff9028ff>${rhsFlat}<> + <#60e84dff>${rhsHpPct}%<> от вашего <$hpIcon> <#60e84dff>максимального здоровья<>, и даёт <#60e84dff>${rhsBonusHpPct}%<> от этого урона как постоянное <#60e84dff>дополнительное здоровье<>."
$i18n.ru.spear_of_shojin.option = "Концентрация: Попадания способностями по вражеским чемпионам дают <#ff9028ff>${sosAtkMult}%<> <$adIcon> <#ff9028ff>Силы Атаки<> на <#e8a800ff>${sosDur} секунд<> (макс. ${sosStacks} стака)."
$i18n.ru.radiant_spear_of_shojin.option = "Концентрация: Попадания способностями по вражеским чемпионам дают <#ff9028ff>${rsosAtkMult}%<> <$adIcon> <#ff9028ff>Силы Атаки<> на <#e8a800ff>${rsosDur} секунд<> (макс. ${rsosStacks} стака)."
$i18n.ru.warmogs_armor.option = "Сердце Вармога: Восстанавливает <#60e84dff>${waHeal}%<> вашего <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>максимального здоровья<> каждую секунду и дает <#ffffffff>${waMs}%<> <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>скорости передвижения<>, если вы не получали урон последние <#e8a800ff>${waDur} секунд<>."
$i18n.ru.radiant_warmogs_armor.option = "Сердце Вармога: Восстанавливает <#60e84dff>${rwaHeal}%<> вашего <i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>максимального здоровья<> каждую секунду и дает <#ffffffff>${rwaMs}%<> <i#asset/base/ui/banpick/champion_stat_icon:speed_0> <#ffffffff>скорости передвижения<>, если вы не получали урон последние <#e8a800ff>${rwaDur} секунд<>."
$i18n.ru.stormrazor.option = "Заряд: Движение и нанесение <#ff9028ff>физического урона<> генерирует заряды <#e8a800ff>Энергии<>, до <#e8a800ff>${srStacks}<>.`n`nМолния: При полном <#e8a800ff>Заряде<>, ваш следующий <#ff9028ff>физический урон<> наносит <#a974ffff>${srDmg} дополнительного магического урона<> и даёт <#ffffffff>${srMs}%<> <$speedIcon> <#ffffffff>скорости передвижения<> на <#e8a800ff>${srDur} секунды<>."
$i18n.ru.radiant_stormrazor.option = "Заряд: Движение и нанесение <#ff9028ff>физического урона<> генерирует заряды <#e8a800ff>Энергии<>, до <#e8a800ff>${rsrStacks}<>.`n`nМолния: При полном <#e8a800ff>Заряде<>, ваш следующий <#ff9028ff>физический урон<> наносит <#a974ffff>${rsrDmg} дополнительного магического урона<> и даёт <#ffffffff>${rsrMs}%<> <$speedIcon> <#ffffffff>скорости передвижения<> на <#e8a800ff>${rsrDur} секунды<>."

$i18n.ru.black_cleaver.option = "Рассечение: Нанесение <#ff9028ff>физического урона<> вражеским чемпионам <#d94c49ff>снижает их <$armorIcon> <#ffdd8eff>броню<> на ${bcShred}%<> на <#e8a800ff>${bcDur} секунд<> (макс. ${bcStacks} стаков)."
$i18n.ru.radiant_black_cleaver.option = "Рассечение: Нанесение <#ff9028ff>физического урона<> вражеским чемпионам <#d94c49ff>снижает их <$armorIcon> <#ffdd8eff>броню<> на ${rbcShred}%<> на <#e8a800ff>${rbcDur} секунд<> (макс. ${rbcStacks} стаков)."
$i18n.ru.bloodletters_curse.option = "Распад: Нанесение <#a974ffff>магического урона<> вражеским чемпионам <#d94c49ff>снижает их <$mrIcon> <#88ccffff>сопротивление магии<> на ${blcShred}%<> на <#e8a800ff>${blcDur} секунд<> (макс. ${blcStacks} стаков)."
$i18n.ru.radiant_bloodletters_curse.option = "Распад: Нанесение <#a974ffff>магического урона<> вражеским чемпионам <#d94c49ff>снижает их <$mrIcon> <#88ccffff>сопротивление магии<> на ${rblcShred}%<> на <#e8a800ff>${rblcDur} секунд<> (макс. ${rblcStacks} стаков)."
$i18n.ru.sundered_sky.option = "Удар Светового Щита: Ваш следующий <#ff9028ff>физический урон<> по вражескому чемпиону <$critIcon> <#d45656ff>наносит критический удар<> с <#e8a800ff>${ssDamage}% дополнительного урона<> и <#60e84dff>восстанавливает вам<> <#60e84dff>${ssFlatHeal}<> + <#60e84dff>${ssPercentHeal}%<> от вашего <#60e84dff>потерянного здоровья<> (перезарядка <#e8a800ff>${ssOnHitCD} секунд<> на каждую цель)."
$i18n.ru.radiant_sundered_sky.option = "Удар Светового Щита: Ваш следующий <#ff9028ff>физический урон<> по вражескому чемпиону <$critIcon> <#d45656ff>наносит критический удар<> с <#e8a800ff>${rssDamage}% дополнительного урона<> и <#60e84dff>восстанавливает вам<> <#60e84dff>${rssFlatHeal}<> + <#60e84dff>${rssPercentHeal}%<> от вашего <#60e84dff>потерянного здоровья<> (перезарядка <#e8a800ff>${rssOnHitCD} секунд<> на каждую цель)."
$i18n.ru.echoes_of_helia.option = "Похищение душ: Накапливает <#e8a800ff>${eohConversion}%<> нанесённого вами урона как <#92dc7bff>Заряды Души<>, до <$levelIcon> <#d8c9b3ff>${eohMinCap}<> - <#d8c9b3ff>${eohMaxCap}<> (в зависимости от <#d8c9b3ff>уровня<>). Попадание умением по вражескому чемпиону расходует все <#92dc7bff>Заряды Души<>, <#60e84dff>восстанавливая здоровье ближайшему союзнику<> на израсходованное количество."
$i18n.ru.radiant_echoes_of_helia.option = "Похищение душ: Накапливает <#e8a800ff>${reohConversion}%<> нанесённого вами урона как <#92dc7bff>Заряды Души<>, до <$levelIcon> <#d8c9b3ff>${reohMinCap}<> - <#d8c9b3ff>${reohMaxCap}<> (в зависимости от <#d8c9b3ff>уровня<>). Попадание умением по вражескому чемпиону расходует все <#92dc7bff>Заряды Души<>, <#60e84dff>восстанавливая здоровье ближайшему союзнику<> на израсходованное количество."

$i18n.ru.sheen.option = "Чародейский клинок: Попадание умением по вражескому чемпиону заставляет вашу следующую атаку нанести <#d8c9b3ff>${sheenMin}<> - <#d8c9b3ff>${sheenMax}<> (в зависимости от <$levelIcon> <#d8c9b3ff>уровня<>) в виде <#ff9028ff>дополнительного физического урона<> (перезарядка <#e8a800ff>${sheenCd} секунд<>)."

$tfTemplateRu = "Чародейский клинок: Попадание умением по вражескому чемпиону заставляет вашу следующую атаку нанести <#ff9028ff>{0}<> + <#ff9028ff>{1}%<> вашей <$adIcon> <#ff9028ff>силы атаки<> в виде <#ff9028ff>дополнительного физического урона<> (перезарядка <#e8a800ff>{2} секунд<>)."
$i18n.ru.trinity_force.option = $tfTemplateRu -f $tfFlat, $tfAdPct, $tfCd
$i18n.ru.radiant_trinity_force.option = $tfTemplateRu -f $rtfFlat, $rtfAdPct, $rtfCd

$dndTemplateRu = "Чародейский клинок: Попадание умением по вражескому чемпиону заставляет вашу следующую атаку нанести <#a974ffff>{0}<> + <#a974ffff>{1}%<> вашей <$apIcon> <#a974ffff>силы умений<> в виде <#a974ffff>дополнительного магического урона<> и <#60e84dff>восстановить вам<> <#a974ffff>{2}%<> вашей <$apIcon> <#a974ffff>силы умений<> и <#60e84dff>{3}%<> вашего <$hpIcon> <#60e84dff>максимального здоровья<> (перезарядка <#e8a800ff>{4} секунд<>)."
$i18n.ru.dusk_and_dawn.option = $dndTemplateRu -f $dndFlat, $dndApPct, $dndApHeal, $dndHpHeal, $dndCd
$i18n.ru.radiant_dusk_and_dawn.option = $dndTemplateRu -f $rdndFlat, $rdndApPct, $rdndApHeal, $rdndHpHeal, $rdndCd

$bsTemplateRu = "Чародейский клинок: Попадание умением по вражескому чемпиону заставляет вашу следующую атаку нанести <#d8c9b3ff>{0}<> - <#d8c9b3ff>{1}<> (в зависимости от <$levelIcon> <#d8c9b3ff>уровня<>) в виде <#a974ffff>дополнительного магического урона<> (перезарядка <#e8a800ff>{2} секунд<>). Если цель — чемпион, увеличивает <#d94c49ff>получаемый ею урон<> на <#d94c49ff>{3}%<> на <#e8a800ff>{4} секунды<>."
$i18n.ru.bloodsong.option = $bsTemplateRu -f $bsMin, $bsMax, $bsCd, $bsAmp, $bsDur
$i18n.ru.radiant_bloodsong.option = $bsTemplateRu -f $rbsMin, $rbsMax, $rbsCd, $rbsAmp, $rbsDur

$lethRu = "Летальность: Игнорирует <#ffdd8eff>{0} <$armorIcon> брони<> при нанесении урона врагам."
$i18n.ru.serrated_dirk.option = $lethRu -f $sdLeth
$hubRu = "$lethRu`n`nВозвышение: При участии в убийстве вражеского чемпиона создаёт постоянный заряд и даёт <#ff9028ff>{1}<> (+{2} за заряд) <#ff9028ff>дополнительной<> <$adIcon> <#ff9028ff>Силы Атаки<> на <#e8a800ff>{3} секунд<>."
$i18n.ru.hubris.option = $hubRu -f $hubLeth, $hubBase, $hubStack, $hubDur
$i18n.ru.radiant_hubris.option = $hubRu -f $rhubLeth, $rhubBase, $rhubStack, $rhubDur
$bbRu = "$lethRu`n`nСаботаж: При участии в убийстве вражеского чемпиона даёт <#92dc7bff>Саботаж<> на <#e8a800ff>{3} секунд<>, усиливая вашу следующую Атаку по башне, чтобы нанести <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> вашей <$adIcon> <#ff9028ff>Силы Атаки<> как <#ff9028ff>дополнительный физический урон<>."
$i18n.ru.bastionbreaker.option = $bbRu -f $bbLeth, $bbFlat, $bbPct, $bbDur
$i18n.ru.radiant_bastionbreaker.option = $bbRu -f $rbbLeth, $rbbFlat, $rbbPct, $rbbDur
$spfRu = "$lethRu`n`nПожиратель щитов: Нанесение урона вражескому чемпиону со щитом наносит дополнительно <#ff9028ff>{1}<> + <#ff9028ff>{2}%<> вашей <$adIcon> <#ff9028ff>Силы Атаки<> как <#ff9028ff>дополнительный физический урон<>."
$i18n.ru.serpents_fang.option = $spfRu -f $spfLeth, $spfFlat, $spfPct
$i18n.ru.radiant_serpents_fang.option = $spfRu -f $rspfLeth, $rspfFlat, $rspfPct

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
Write-Host "  Death's Dance:           ${ddDelay}% deferred / ${ddBurnCap}% max HP per second / ${ddFlatHeal}+${ddHeal}% missing HP heal on takedown"
Write-Host "  Radiant Death's Dance:   ${rddDelay}% deferred / ${rddBurnCap}% max HP per second / ${rddFlatHeal}+${rddHeal}% missing HP heal on takedown"
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
Write-Host "  Terminus:               ${tArmorPen}%/${tMagicPen}% pen/stack / ${tDur}s / ${tStacks} stacks"
Write-Host "  Radiant Terminus:       ${rtArmorPen}%/${rtMagicPen}% pen/stack / ${rtDur}s / ${rtStacks} stacks"
Write-Host "  Collector:              executes below ${colThreshold}% HP"
Write-Host "  Radiant Collector:      executes below ${rcolThreshold}% HP"
Write-Host "  Heartsteel:             ${hsFlat} + ${hsHpPct}% max HP dmg / ${hsBonusHpPct}% bonus HP / ${hsCd}s cooldown"
Write-Host "  Radiant Heartsteel:     ${rhsFlat} + ${rhsHpPct}% max HP dmg / ${rhsBonusHpPct}% bonus HP / ${rhsCd}s cooldown"
Write-Host "  Spear of Shojin:         ${sosAtkMult}% AD/stack / ${sosDur}s / ${sosStacks} stacks"
Write-Host "  Radiant Spear of Shojin: ${rsosAtkMult}% AD/stack / ${rsosDur}s / ${rsosStacks} stacks"
Write-Host "  Warmog's Armor:          ${waHeal}% max HP/s / ${waMs} movement speed / ${waDur} out of combat cooldown"
Write-Host "  Radiant Warmog's Armor:  ${rwaHeal}% max HP/s / ${rwaMs} movement speed / ${rwaDur} out of combat cooldown"
Write-Host "  Shadowflame:             +20% AP dmg vs targets below ${sfThreshold}% HP"
Write-Host "  Radiant Shadowflame:     +20% AP dmg vs targets below ${rsfThreshold}% HP"
Write-Host "  Yun Tal Wildarrows:         +${ytCrit}% crit/hit (cap ${ytMaxCrit}%) / +${ytFlurryAS}% AS ${ytDur}s (${ytCd}s CD)"
Write-Host "  Radiant Yun Tal Wildarrows: +${rytCrit}% crit/hit (cap ${rytMaxCrit}%) / +${rytFlurryAS}% AS ${rytDur}s (${rytCd}s CD)"
Write-Host "  Stormrazor:              ${srDmg} magic dmg / ${srMs}% MS ${srDur}s / ${srStacks} stacks"
Write-Host "  Radiant Stormrazor:      ${rsrDmg} magic dmg / ${rsrMs}% MS ${rsrDur}s / ${rsrStacks} stacks"
Write-Host "  Black Cleaver:               -${bcShred}% armor / ${bcDur}s / ${bcStacks} stacks"
Write-Host "  Radiant Black Cleaver:       -${rbcShred}% armor / ${rbcDur}s / ${rbcStacks} stacks"
Write-Host "  Bloodletter's Curse:         -${blcShred}% MR / ${blcDur}s / ${blcStacks} stacks"
Write-Host "  Radiant Bloodletter's Curse: -${rblcShred}% MR / ${rblcDur}s / ${rblcStacks} stacks"
Write-Host "  Sundered Sky:                ${ssDamage}% first hit damage bonus / ${ssFlatHeal} + ${ssPercentHeal}% missing HP heal / (${ssOnHitCD} CD per target)"
Write-Host "  Sundered Sky:                ${rssDamage}% first hit damage bonus / ${rssFlatHeal} + ${rssPercentHeal}% missing HP heal / (${rssOnHitCD} CD per target)"
Write-Host "  Bloodsong:                   ${bsMin} - ${bsMax} magic (by level) / ${bsCd}s CD / +${bsAmp}% damage taken ${bsDur}s"
Write-Host "  Radiant Bloodsong:           ${rbsMin} - ${rbsMax} magic (by level) / ${rbsCd}s CD / +${rbsAmp}% damage taken ${rbsDur}s"
