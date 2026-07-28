use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, is_enemy_champion, ticks};
use mod_api::*;

const RAGE_BUFF: &str = "phage_rage";

#[derive(Clone, Debug)]
pub struct Phage {
    price: usize,
    hp: i32,
    attack: i32,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
}

impl Default for Phage {
    fn default() -> Self {
        Self {
            price: 950,
            hp: 200,
            attack: 30,
            effect_move_speed_mult: 5,
            effect_duration_seconds: 2.0,
        }
    }
}

impl Phage {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                hp,
                attack,
                effect_move_speed_mult,
                effect_duration_seconds
            ]
        );
        item
    }

    /// Rage. Re-applies only once the previous burst has lapsed: same-name buffs
    /// stack rather than refresh, so an unguarded grant would pile movement speed
    /// up with every hit.
    fn grant_rage(&mut self, ctx: &mut GameCtx, caster: usize, target: usize) {
        if !is_enemy_champion(ctx, caster, target) {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        if has_buff(&caster_ref, RAGE_BUFF) {
            return;
        }
        ctx.add_buff(
            caster,
            BuffState {
                name: buff_name(RAGE_BUFF),
                duration: BuffType::Time {
                    tick: ticks(self.effect_duration_seconds),
                },
                move_speed_mult: self.effect_move_speed_mult,
                ..Default::default()
            },
        );
    }
}

impl ModItemInfo for Phage {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "phage"
    }

    fn icon(&self) -> &str {
        "phage"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "hardened_heart".to_string(),
            "soldiers_longsword".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "frozen_mallet".to_string(),
            "overlords_bloodmail".to_string(),
            "black_cleaver".to_string(),
            "trinity_force".to_string(),
        ]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        self.grant_rage(ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::HP, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}
