use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta, SelfCastWatch};

#[derive(Clone, Debug)]
pub struct EchoesOfHelia {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_damage_conversion: f64,
    effect_min_stacks: usize,
    effect_max_stacks: usize,
    charge_stored: usize,
    self_cast: SelfCastWatch,
}

impl EchoesOfHelia {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "echoes_of_helia",
                &["bandleglass_mirror", "forbidden_idol"],
                &["radiant_echoes_of_helia"],
            ),
            price: 550,
            hp: 150,
            hp_regen: 2,
            magic_power: 25,
            skill_cooldown_mult: 15,
            effect_damage_conversion: 30.0,
            effect_min_stacks: 130,
            effect_max_stacks: 350,
            // Non-vital stats (internals)
            charge_stored: 0,
            self_cast: SelfCastWatch::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_echoes_of_helia", &["echoes_of_helia"]),
            price: 750,
            hp: 250,
            hp_regen: 3,
            magic_power: 35,
            skill_cooldown_mult: 20,
            effect_damage_conversion: 30.0,
            effect_min_stacks: 130,
            effect_max_stacks: 350,
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
                hp,
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_damage_conversion,
                effect_min_stacks,
                effect_max_stacks
            ]
        );
        self
    }

    pub fn save_charges(&mut self, level: usize, damage: f64) {
        let stack_gain = (damage * (self.effect_damage_conversion / 100.0)) as usize;
        let limit_per_level = (self.effect_max_stacks - self.effect_min_stacks) as f64 / 11.0;
        let max_limit = self.effect_min_stacks + (level - 1) * limit_per_level.round() as usize;

        if self.charge_stored + stack_gain > max_limit {
            self.charge_stored = max_limit;
        } else {
            self.charge_stored += stack_gain;
        }
    }

    // Spends every stored charge on `target`, a living allied champion.
    fn spend_charges(&mut self, ctx: &mut StableSim<'_>, caster: usize, target: usize) {
        ctx.heal(caster, target, self.charge_stored);
        self.charge_stored = 0;
    }
}

impl Default for EchoesOfHelia {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for EchoesOfHelia {
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
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        self.save_charges(entity_ref.level(), damage as f64);
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        self.save_charges(caster_ref.level(), *damage as f64);
    }

    // Spends the stored charges on an ally.
    //
    // Self-casts count as ally-targeted, and Soul Charges only ever spend on
    // someone else, so a self-cast spends nothing here. It may still have
    // healed an ally, as the Monk's heal does around them: that is watched for
    // and spent in `update`.
    //
    // Every one of these checks now returns *before* the reset rather than
    // skipping only the heal. Clearing the charges unconditionally spent them
    // on casts that healed nobody — a self-cast, an enemy-targeted skill, a
    // minion, a dead ally — which contradicted the tooltip and made the
    // self-exclusion a punishment rather than a no-op.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if !is_ally {
            return;
        }
        if target == caster {
            self.self_cast.open(ctx, caster);
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || !target_ref.is_alive() {
            return;
        }

        self.spend_charges(ctx, caster, target);
    }

    // A self-cast that healed allies as well spends the charges on the most
    // wounded of them: the charges are spent whole, on one ally.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let healed = self.self_cast.poll(ctx);
        let most_wounded = healed
            .into_iter()
            .filter_map(|id| {
                let (current, max) = ctx.get_entity(id)?.hp();
                (max > 0).then(|| (id, current as f64 / max as f64))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((target, _)) = most_wounded else {
            return;
        };
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|champion_ref| champion_ref.id())
        else {
            return;
        };

        self.spend_charges(ctx, caster, target);
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.self_cast.close();
    }

    // Stored charges follow the item through the Radiant upgrade; the next
    // `save_charges` clamps them back down if the wielder's level allows less.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.charge_stored as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.charge_stored = carry as usize;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::Vamp,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}
