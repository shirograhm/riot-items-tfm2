use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

// Lifeline: If you would take damage below 30% of your maximum health, you first
// gain a shield that absorbs damage equal to 60% of your maximum health for 4
// seconds (90 second cooldown).
#[derive(Clone, Debug)]
pub struct SteraksGage {
    meta: ItemMeta,
    cooldown_buff: &'static str,
    price: usize,
    attack: i32,
    hp: i32,
    toughness: usize,
    effect_hp_percent_threshold: f64,
    effect_caster_hp_percent_shield: f64,
    effect_shield_seconds: f64,
    effect_cooldown_seconds: f64,
}

impl SteraksGage {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("steraks_gage", &["phage"], &["radiant_steraks_gage"]),
            cooldown_buff: "steraks_gage_cooldown_buff",
            price: 1400,
            attack: 30,
            hp: 400,
            toughness: 15,
            effect_hp_percent_threshold: 30.0,
            effect_caster_hp_percent_shield: 60.0,
            effect_shield_seconds: 4.0,
            effect_cooldown_seconds: 90.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_steraks_gage", &["steraks_gage"]),
            cooldown_buff: "steraks_gage_cooldown_buff",
            price: 2000,
            attack: 50,
            hp: 650,
            toughness: 20,
            effect_hp_percent_threshold: 30.0,
            effect_caster_hp_percent_shield: 60.0,
            effect_shield_seconds: 4.0,
            effect_cooldown_seconds: 90.0,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                attack,
                hp,
                toughness,
                effect_hp_percent_threshold,
                effect_caster_hp_percent_shield,
                effect_shield_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for SteraksGage {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SteraksGage {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            hp: self.hp,
            toughness: self.toughness,
            ..Default::default()
        }
    }

    // The host resolves the hit before this runs, so "damage that would take you
    // below the threshold" is read after the fact: the shield lands on the tick
    // the carrier crosses under it, and covers what comes next.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        if has_buff(&entity_ref, self.cooldown_buff) {
            return;
        }

        let (current_hp, max_hp) = entity_ref.hp();
        if current_hp > percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }

        let shield = percent_of(max_hp, self.effect_caster_hp_percent_shield);
        if shield == 0 {
            return;
        }

        ctx.entity_add_shield(entity, shield, ticks(self.effect_shield_seconds));
        ctx.add_buff(
            entity,
            &BuffV1::timed(self.cooldown_buff, ticks(self.effect_cooldown_seconds)),
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ad,
            ItemTagV1::Toughness,
            ItemTagV1::Shield,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
