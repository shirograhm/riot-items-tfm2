# Merging `tfm2_item_tactics` into `riot_items_tfm2`

`tfm2_item_tactics` (by tfm2mods) was a standalone mod that raises the game's
item slots from three to four. As of 2026-08-04 its source lives in
[`src/tactics`](../src/tactics) and ships inside this mod. Its own design notes
are preserved verbatim in [`item_tactics_HOW_IT_WORKS.md`](item_tactics_HOW_IT_WORKS.md)
— read that for *what* the code does. This file records only what the merge
changed and what it left unresolved.

## Why the classic scaffolding had to go

The two mods used mutually exclusive entry points:

* this mod exports `tfm2_mod_entry_stable` (`declare_stable_mod!`);
* `tfm2_item_tactics` exported the classic one (`declare_mod!`).

A DLL gets one. From `mod-api-stable/src/entry.rs`: the stable symbol being
present "=> the DLL is a stable-ABI mod and the loader must not use the legacy
exact-match path". So the classic entry could never have been called, and
`init`, `ModExtension` and `ModServerExtension` were rewritten as plain
functions driven from this mod's stable extensions.

Going the other way — making the merged DLL classic — was rejected because it
means re-expressing every `StableItem` in `src/items` (100+ files) against the
classic item API, to gain nothing the substitutes below don't already provide.

## The three substitutes

`src/tactics/driver.rs` is the whole adapter. Everything else in `src/tactics`
is raw pointers and kernel32 and needed no change at all.

| classic API | why the stable ABI can't provide it | substitute |
|---|---|---|
| `ServerModContext::database` | no object pointers cross the boundary | the item network address, which `src/hook.rs` is handed as `&LogisticSGDAgent`; the `Database` base is that minus `0x1558` |
| `&mut GameUI` (`ui.root`) | `StableClient` exposes UI by path, not as a node tree | **nothing — this substitute was wrong, see below**; the tree walking is disabled by `UI_TREE_WALK_ENABLED` |
| `Scene::InGame { data }` | no scene payload crosses the boundary | `StableClient::is_in_game()`, threaded in as a `bool` |

Two consequences worth knowing:

* **The `Database` is not available at server start.** It arrives the first time
  the game asks for an item build. `probe_db` no-ops until then and retries from
  `before_management_tick`, which is why that hook now exists.
* **The hook-install retry moved to the top of `tactics_post_update`.** It is
  what installs `cap_game_view`, and `cap_game_view` is what publishes
  `TIP_ROOT` — so the "no root, return early" guard has to sit *after* it. The
  other order deadlocks: the hook is never installed, the root never captured,
  and every frame returns early forever.

`mod_api` is still linked, for its types only (`Node`, and the glob the file
was written against). Same justification as `game_core` in `src/hook.rs`:
`repr(Rust)` layout is fixed by the compiler, and `rust-toolchain.toml` pins the
compiler the game is built with.

## Other changes

* `mod_dir()` was `<game>/mods/tfm2_item_tactics`, which no longer exists. It is
  now `config::dll_dir()` — this mod's own folder, wherever it is installed.
  That also fixes the Workshop case the old expression got wrong (a subscribed
  mod lives under `steamapps/workshop/content/<appid>/<published_file_id>/`).
* `companion.rs` detected the separately installed `tfm2_item_tactics` to decide
  whether to offer a fourth slot in the build editor. That detection is kept —
  a user may still have that mod installed alongside — but the merged half's own
  answer now takes precedence, via `record_builtin_item_slots`.
* `4items.cfg` ships with `slots = 3`, i.e. every four-item behaviour off. This
  combination has never been run.

## Three bugs the merge introduced, and what they taught

All three were substitutions that looked equivalent and were not. Recorded
because each failed *silently*, and the silence was the expensive part.

### 1. `TIP_ROOT` is not `GameUI.root`

The first merged build crashed on startup in *both* slot modes, with the game log
stopping at "paragraph paint done" — the first UI frame.

`TIP_ROOT` is `arg4` of the UI mega-function, captured so it can be handed back
to the game's own tooltip `show` function as its node search root. The original
mod never treated it as a `Node`. Reinterpreting it as one made `find_node`
follow child pointers that are not child pointers — an access violation, which
`catch_unwind` cannot catch, so the process dies rather than degrading.

Fixed by `src/tactics/ui_root.rs`, which *finds and validates* the root instead:
`TIP_ROOT` first (free to test), then a window scan from `App`
(`GAME_VIEW - 0x4a50`) testing each slot as both a pointer to a `Node` and an
inline one. A candidate is accepted only after a `safe_read`-only walk proves it
has a readable ASCII id and a coherent child vector containing `main`. Field
offsets come from `offset_of!` on `mod_api`'s `Node`, so they track the type.

**The rule:** the VEH only rewrites faults inside `safe_copy`'s asm block, so a
bad pointer anywhere else kills the process. Never hand an unvalidated address to
`find_node`.

### 2. Disabling the UI walk disabled every `safe_read_*`

While the walk was gated off, the 4th item was never bought, and every diagnostic
read healthy: hooks "installed", counters zero.

`seh_install()` registers the VEH that gates `safe_copy`, which returns `false`
on entry until it runs. Upstream called it from exactly two places, and the one
that mattered was inside `handle_tactics_screen` — the VEH was registered as a
*side effect* of a UI handler running every frame. Gating that off meant
`install_launcher_hook` returned at its first `safe_read_u64` on all 18,902
calls (`LIVE_SEED` stayed 0, so no buy was ever classified as live), and
`is_my_athlete` could not read `athlete+0x810`, so the buy detour exited before
touching a build.

`seh_install()` now runs first thing in `tactics_init`.

### 3. The `Database` derivation validates itself

`record_item_net` derives `Database = item_network - 0x1558`. Its check —
`sig_ok(db + 0x1558)` — is true by construction, so a wrong base *passes*.
`dump_mod_items` scanned from it and found 0 mod items, leaving `auto_cands()`
as `VANILLA_FINAL` alone: the auto-picked 4th item could only ever be vanilla.

Replaced by `record_item_catalog`, fed from the `&Vec<Box<dyn ItemInfo>>` that
`src/hook.rs` already receives. That is the list the game is actually using, it
arrives typed, and it needs no base address. `db_addr()` is still derived and
still used by `probe_db` for the item network, but nothing depends on the memory
scan any more.

Finals use upstream's two-pass rule — `next_tier` empty **and** something
upgrades into it — because "no next tier" alone also accepts a base component
nothing builds into, which is not a legal build goal.

### The pattern behind all three

Every one was a *latching* failure: `MODITEMS_DONE`, `AUTO_CANDS` and `SLOTS` all
memoize on first call, so a wrong early answer is permanent. When adding a cache
here, cache only *validated* answers — which is what `ui_root::resolve` and
`companion::item_slots` now both do.

## Status

Working in-game as of 2026-08-04 with `slots = 4`: the 4th item is bought, its
stats apply, and its icon and tooltip render in-match. The auto-picked 4th item
can be a mod item.

`BUILD_EXT_DIAG` in `src/tactics/mod.rs` is the diagnostic that established all
of this — off in production, one flip to get `build_ext_diag.txt` back. It is the
first thing to reach for if any of this regresses.

## Still unresolved

1. **The strategy screen collides.** This mod's `ui/layout/strategy` override
   *replaces the Personal tab with `#builds`* (see the module docs in
   `src/strategy_ui.rs`). The tactics half's `inject_strategy` appends its
   `item0m`/`item1m`/`item2m`/`item3` dropdowns to `row0..row4` under
   `#personal`, and `handle_tactics_screen` gates on `#personal` being visible.
   On this mod's strategy screen there is no `#personal`, so that injection
   finds nothing to attach to and the tactics designation UI does not appear.
   The engine half (purchasing, the fourth slot, the in-match icon) is
   unaffected. Resolving this means picking one of: restore `#personal` in
   `ui/layout/strategy.ui`, or let this mod's own Builds editor be the
   designation UI and drop the injection.
2. **`player_info` is hand-authored, not generated.** `ui_inject` replaces the
   root's children wholesale with an embedded `.ui` that is the base file plus a
   4th slot and tighter spacing. That delta has to be re-applied by hand when the
   game ships a new base — the same rot an override suffers (see
   `item_tactics_HOW_IT_WORKS.md` §7.1), just confined to one file nobody else
   touches. Generating it instead would mean appending a `slot3` node to the
   existing container at runtime and letting `force_blue_slot_spacing` (which
   already re-spaces `base + 42*i` for `i in 0..4` every frame) do the layout.
   That would delete both `.ui` files and survive game updates.

   Note these must **not** be listed in `mod.override_info`. They were during
   the merge, and because an override is applied before any mod code runs, it
   cannot be conditional: `slots = 3` still got the 4-slot layout, defeating the
   `mode4` gate in `loader_body`. Removed 2026-08-04.
3. **The direct scene read is off.** `LIVE_DB` was the `ClientDatabase` pointer,
   and `quick_scene_side` reads the live scene's team ids out of it to decide
   which sim side is the player's. Nothing leaks that pointer to a stable-ABI
   mod, so `SCENE_SIDE` stays undetermined and the team gate falls back to the
   roster (`MY_ATHLETES`, published from the stable record API) — which is a
   supported path, not a break. What is lost is the spawn hook's early side
   decision, which covered the `owned=0` injection window.
4. **Two detours, untested together.** This mod hooks the item-build route
   function; the tactics half hooks `buy_item`. Different functions, so they
   should not fight — but the previously recorded risk of the two halves
   disagreeing about a build has never been tested in-game.
5. **`tfm2_item_tactics` shipped a `ui/layout/strategy.ui`** that its own
   `mod.override_info` (`{}`) never referenced — it injects at runtime instead.
   This mod's `ui/layout/strategy.ui` was kept and theirs was not copied in.
