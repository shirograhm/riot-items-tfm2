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

## The UI-root substitution was wrong

The first merged build crashed the game on startup in *both* slot modes, with
the game log stopping at "paragraph paint done" — the first UI frame.

`TIP_ROOT` is `arg4` of the UI mega-function, captured by `cap_game_view` so it
can be handed straight back to the game's own tooltip `show` function as its
node search root. The original mod never treated it as a `Node`. Reinterpreting
it as `GameUI.root` made `find_node` follow child pointers that are not child
pointers, which is an access violation — and `catch_unwind` cannot catch one, so
the process dies rather than degrading.

`UI_TREE_WALK_ENABLED` (in `src/tactics/mod.rs`, currently `false`) gates every
handler that starts from that pointer. Its doc comment lists exactly what is off
and the two routes to turning it back on. The engine half — byte patches,
`buy_item` injection, mod-item scan, neural 4th-item pick, and `ui_inject`'s
loader hook — is unaffected, because none of it touches the live node tree.

The visible cost is the **in-match 4th item icon and its tooltip**. The 4th item
is still bought and its stats still apply; it is just not drawn.

## Unresolved — read before enabling `slots = 4`

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
2. **`player_info` is now delivered twice.** `mod.override_info` remaps
   `player_info` and `wide_player_info` to the tactics versions, and
   `ui_inject.rs` *also* replaces those templates through its loader hook
   (`IN_MATCH_UI`). Both write the same content, so this should be redundant
   rather than wrong — but it has not been observed.
3. **The `Database` derivation is the weakest link.** `record_item_net` assumes
   the `&LogisticSGDAgent` this mod's item-build detour receives is the same
   network `tfm2_item_tactics` locates at `Database + 0x1558`. It validates the
   header (`16384/16384/1`) and the weight pointer before believing it, so a
   wrong guess fails closed — but "fails closed" here means the mod-item scan
   and the neural fourth-item pick silently never run. If the fourth item comes
   out as the vanilla fallback, check this first.
4. **Two detours, untested together.** This mod hooks the item-build route
   function; the tactics half hooks `buy_item`. Different functions, so they
   should not fight — but the previously recorded risk of the two halves
   disagreeing about a build has never been tested in-game.
5. **Nothing here has been compiled.** The merge is source-complete; the build
   is yours to run.
6. **`tfm2_item_tactics` shipped a `ui/layout/strategy.ui`** that its own
   `mod.override_info` (`{}`) never referenced — it injects at runtime instead.
   This mod's `ui/layout/strategy.ui` was kept and theirs was not copied in.
