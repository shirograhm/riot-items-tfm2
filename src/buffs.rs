use mod_api_stable::*;

pub(crate) fn refresh_buff(ctx: &mut StableSim<'_>, entity: usize, name: &str, buff: &BuffV1) {
    ctx.entity_remove_buff(entity, name);
    ctx.add_buff(entity, buff);
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Stacks {
    entries: Vec<StackEntry>,
}

#[derive(Clone, Copy, Debug)]
struct StackEntry {
    entity: usize,
    count: usize,
    remaining: usize,
}

impl Stacks {
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds one stack on `entity` (capped at `max`) and restarts the shared
    /// duration. Returns the new count; `0` only when `max` is `0`.
    pub(crate) fn add(&mut self, entity: usize, max: usize, duration: usize) -> usize {
        if max == 0 {
            return 0;
        }
        match self.entries.iter_mut().find(|e| e.entity == entity) {
            Some(entry) => {
                entry.count = (entry.count + 1).min(max);
                entry.remaining = duration;
                entry.count
            }
            None => {
                self.entries.push(StackEntry {
                    entity,
                    count: 1,
                    remaining: duration,
                });
                1
            }
        }
    }

    /// Advances every entry by one tick and drops the ones that ran out.
    pub(crate) fn tick(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.entries.retain_mut(|entry| {
            entry.remaining = entry.remaining.saturating_sub(1);
            entry.remaining > 0
        });
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}
