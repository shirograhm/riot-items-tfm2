# tfm2_item_tactics — How It Works

A native (Rust `cdylib`) mod for **Teamfight Manager 2 0.5.3** that

* extends every athlete's item build from **3 slots to 4**,
* lets you **designate a specific item** (vanilla *or* mod-added) for **each of the 4 slots** on the
  Personal Tactics screen, and
* leaves any slot you did not designate to the **game's own item recommendation network**.

This document explains the *mechanisms*, not the UI. It is written for people who want to build a
similar mod: the interesting parts are **when** a designation is applied, **how the 4th item is
chosen and actually purchased**, **how mod item IDs are discovered at runtime**, and **why the UI is
injected through loader hooks instead of the game's asset override system**.

> Version note: every hardcoded RVA in the source is for game **0.5.3** (exe 74,970,624 B).
> The mod refuses to install a single hook on any other build — see [Version gate](#9-version-gate).

---

## 1. The central design decision: plant a *goal*, never force a purchase

The mod **does not buy items**. It writes into the athlete's **`build[]` array** — the list of
*target* items that the game's own purchase resolver works towards — and then gets out of the way.

```
athlete + 0x490  build Vec cap
athlete + 0x498  build Vec ptr     elements = 8 bytes = a CATALOG INDEX (not an item id)
athlete + 0x4a0  build Vec len     vanilla = 3, this mod grows it to 4

athlete + 0x448  items Vec cap     the items actually OWNED
athlete + 0x450  items Vec ptr     elements = 16 bytes = (object ptr, vtable)
athlete + 0x458  items Vec len     == "owned"
```

`build` = *what I am trying to buy*. `items` = *what I own*. Confusing the two is the single most
common mistake when reading this code.

Because only the goal is planted, the game keeps doing what it always does: buy the cheapest useful
component, combine upward, pay the real gold price. Two consequences that look like bugs but are not:

* **Slots do not complete in order 1 → 2 → 3 → 4.** All four goals are present from the first
  second of the match, so the resolver may buy a component for slot 4 before slot 3 is finished.
* **Your team and the enemy team behave differently.** Enemy athletes have no designation, so their
  targets are produced one at a time by the game's recommender. Yours are all pinned up front.

Two byte patches make a 4th slot possible at all:

| Patch | What it changes | Without it |
|---|---|---|
| `patch_gate3` | in the resolver, `jbe` → `jmp` (`cmp qword[rsp+0x40],2; jbe`) so the "owned > 2" branch takes the same natural build-up path as slots 1–3 | the 4th slot is never purchased (0 purchases) |
| `patch_owned_cap` | `cmp qword[reg+0x458], 3` immediate `3` → `4` in `run_tick_ext` | the 4th item is owned but its **stats never apply** |

Both sites are validated byte-for-byte before being written; a signature mismatch skips the patch
instead of corrupting code.

---

## 2. When a designation is applied (the timing question)

There is exactly **one** injection point at runtime: a detour on the game's **`buy_item`**
function (`RVA_BUY_ITEM = 0xd0c680`, argument `r8 = athlete`). It fires every time any athlete in
any simulation considers buying something — including background league sims, which is why the
early-exit ordering in that detour matters so much (see [§7](#7-performance-notes)).

Inside the detour, in this order:

```
buy_item(athlete, …)
 │
 ├─ (a) is this an on-screen match?          provider (r9) seed == LIVE_SEED   → is_live
 ├─ (b) is this athlete one of mine?         athlete+0x810 ∈ MY_ATHLETES       → is_player
 │        (comp test bypasses this: both sides are user-composed)
 ├─ (c) which champion is this?              athlete+0x420/0x428 (name String)
 ├─ (d) look up SEL[(scope, champion, slot)] for slots 0..3
 │
 ├─ slots 0/1/2:  if owned <= slot           build[slot] ← catalog index of the designated item
 │                (already-bought slots are skipped — too late to change them)
 │
 └─ slot 3:       if build.len == 3 && cap == 3
                     realloc the build Vec 3 → 4  (via the game's own __rust_realloc)
                     build[3] ← designated item, else neural recommendation, else vanilla fallback
```

So the practical answer to *"when is my choice applied?"* is:

* **Per athlete, per buy call, as long as that slot has not been bought yet.**
  A designation made on the Strategy screen before kick-off applies from the first buy.
* Changing a designation **mid-match** affects only slots the athlete has not filled yet.
* The write is **idempotent** — if `build[slot]` already holds the target index nothing is written.
  (Measured: 53,890 writes collapsed to about 10 once the value comparison was added.)

### Where the designation is stored

`item_tactics_sel.txt`, one line per entry: `champ slot token`.

* `token` = `1`–`6` for a vanilla category, or the **item key string** for a mod item, or `auto`.
* Storing the *key*, not the dropdown index, is deliberate: option indices shift whenever you enable
  or disable an item mod, and the old index-based format silently remapped every saved choice to a
  different item.
* `champ` may carry a **scope prefix**: none = normal play (league / spectate / background),
  `@b:` = comp test blue side, `@r:` = comp test red side. Lookup is scope-first with a fallback to
  the unprefixed key, so an old file still works and comp-test choices never leak into league play.

---

## 3. The item recommendation network, and why the mod touches it

### 3.1 Why touch it at all

The game ships a small trained neural network that scores item builds. It is what decides an
athlete's items when the player does not micromanage them. The problem for this mod:

**The game's recommender only ever produces a 3-item build.** Its beam search runs to depth 3, and
the code that reads a build out is written for 3 entries. If the mod simply extended `build[]` to
length 4 and left slot 3 empty, the resolver would have nothing to aim at and would buy nothing
for it.

So the mod needs a *target* for slot 3 whenever the user did not pick one. There were three options:

1. Hardcode a fallback item — produces identical, obviously wrong builds (early versions always
   picked attack damage, so every enemy's 4th item was an AD item).
2. Patch the game's beam search to depth 4 — tried (`AUTO4_BEAM_DEPTH`), and it broke build
   generation entirely (`beam4` output dropped to 0). Abandoned.
3. **Call the network directly, once, at the moment a 4th item is needed.** This is what ships.

Option 3 is the only one that produces a 4th item that is *consistent with the game's own taste* —
same weights, same features, same lineup context — so a modded match still looks like a normal match.

### 3.2 How the 4th item is chosen

At `owned == 3`, `compute_auto_4th_id()` runs a one-step sweep:

```rust
forward(net, ctx: &[u64; 11], build_ptr, build_len, flag = 0) -> f32   // RVA 0x10587e0
```

* `net` — the trained network, found at `Database + 0x1558` (validated by its header
  `16384 / 16384 / 1` **and** by checking that its weight pointer is readable).
* `ctx[0..5]` — our team's champion ids, indexed **by position**, `ctx[5..10]` — the opponents',
  `ctx[10]` — this athlete's position (0–4). Above 4 the game's own `forward` panics.
  The lineup is reconstructed from the match's roster array (`SimState + 0x840`, stride `0x8d0`);
  each parallel background match has its own array, so there is no cross-match contamination.
* The sweep evaluates `[build0, build1, build2, candidate]` for every candidate final item and keeps
  the highest score. That score *is* the network's answer to "what should the 4th item be".

Guards, all learned the hard way:

* The weight pointer is **re-validated on every call**. It goes stale across session switches, and
  dereferencing it inside `forward` produced an access violation (0xC0000005) that no `catch_unwind`
  can catch. Stale → skip the call and fall back.
* Results are cached under the key `(champion, build[0..3], lineup ctx)`. Same match, same champion,
  same partial build ⇒ same answer, so the ~51 `forward` calls per decision happen once instead of
  on every repeated buy tick.
* If the network is unavailable, the fallback picks a vanilla final item distinct from
  `build[0..2]`, starting from an offset derived from an FNV hash of the champion name — deterministic
  (replay-safe) but spread across categories instead of always landing on the same item.

### 3.3 "The 4th item and beyond"

The array is grown to exactly 4 (`ITEM_SLOTS = 4`). Everything above 3 rides on the same three
mechanisms, so extending further is mostly a matter of repeating them:

1. `build[]` reallocated to the new length, one goal per slot;
2. the resolver's `owned > 2` recipe gate neutralised (`patch_gate3`) so components can still be
   bought;
3. the stat-application cap raised (`patch_owned_cap` `3 → 4`).

The limit today is not the purchase logic but the **UI**: the in-match slot rendering loop has a
hardcoded 3-element node-name array, which is why the 4th icon is drawn by the mod itself
(see [§6](#6-in-match-display-reading-the-view-model)).

### 3.4 Offline: dumping what the network currently recommends

Dropping a file named `dump_builds.trigger` into the mod folder and entering the management screen
scores every (champion × position) combination and writes `item_builds_<ms>.csv` (top 3 builds each),
then deletes the trigger. It runs on the management tick, never during a sim, so the shadow-call
cannot race the simulation. Exhaustive search is used while the projected `forward` call count stays
under 1.5 M; above that it falls back to a width-32 beam **and says so in the CSV header** — silent
truncation is not allowed.

Caveat worth repeating: `forward` contains exploration noise of `U[0,1)*0.2 − 0.1`. Set
`noise_range = 0` / `noise_offset = 0` (in `tfm2_itemnet_tune`) before dumping, or the scores move
between runs. The dump scores one build twice and stamps a warning into the CSV if they disagree.

---

## 4. Finding mod items and their IDs at runtime

Item IDs are not stable data you can read from a file. A mod's items are merged into the game's
item collection at load time, and the resulting numeric id depends on what is loaded. So the mod
discovers them by **scanning the live `Database` object** (`dump_mod_items`, once at server start).

### 4.1 Locating the array

There is no symbol for `mod_items`. The scan walks `Database + 0 .. 0x60000` in 8-byte steps and, at
each offset, tries the four plausible `(ptr, count)` pairings of three consecutive words — a Rust
`Vec` in memory is three words and the field order is not something to assume. A candidate is
accepted only if all of the following hold:

* the pointer looks like a heap address and `3 ≤ count ≤ 2000`;
* an **item key string** can be read at `element + 0x8` (ASCII identifier, ≥ 3 chars);
* an element **stride** can be detected from `{0x1a8, 0x198, 0x1b0}` such that four consecutive
  elements yield four *distinct* keys;
* at least 80 % of the first 48 elements yield a readable key;
* the first key is **not** a vanilla key (`VANILLA_KEYS`, the 30 base items) — the vanilla array is a
  different array and must not be mistaken for this one.

Every read goes through a VEH-guarded `safe_read_*`, so a bad guess returns `None` instead of
crashing the game.

### 4.2 Picking the right candidate

Several arrays survive that filter (mod champions and mod players look structurally similar). Two
tie-breakers, in order:

1. **`next_tier` votes.** Only an *item* has a `next_tier` list (the items it builds into). The
   scan probes candidate offsets for a `Vec<String>` of length ≤ 8 whose elements are readable item
   keys, and scores each array by how many of its elements have one. A champion array scores ~0.
   This fixed an early bug where a larger mod-champion array was chosen over the item array.
2. **Number of *active* entries** (see §5). The array the game actually uses is the one that
   contains active items. Ties fall back to the larger element count.

The result is published as:

```
MOD_REGISTRY[i] = key        // and the in-game item ID is 30 + i (30 vanilla items come first)
MOD_BUF                      // array base; element = MOD_BUF + i * stride, key at element + 0
MOD_FINALS                   // ids of items that are "final" (see below)
```

### 4.3 Which mod items are *final* items

Only final items make sense as a build goal. The rule is a two-pass one:

* **Pass 1** collects `built_set` = the union of every `next_tier` target across the array.
* **Pass 2** keeps item `i` if its own `next_tier` is **empty** *and* it appears in `built_set`.

Both halves matter. `next_tier` empty alone would also accept a base component that nothing builds
into (e.g. `needlessly_large_rod`); membership in `built_set` proves something upgrades into it.

One subtlety worth copying: `read_nt` returns `Option<Vec<_>>`, and `None` (offset not readable as a
`next_tier` at all) is **excluded** from finals rather than treated as an empty list. The earlier
`unwrap_or_default()` conflated "no next tier" with "could not tell", which produced wrong final
items in exactly the case where it hurts — when an override is in play.

### 4.4 From a key to a `build[]` value

A `build[]` entry is a **catalog index**, not an item id, and for mod items the two differ. So when
a designation names a mod item, the mod scans the live catalog collection
(`ctx + 0x30`; elements are `{object ptr, vtable}` with stride `0x10`), reads each element's name
through `vtable[0x50]`, and takes the index of the match. Before using it, `vtable[0x70]` is called
to confirm the item **has a recipe** — writing a recipe-less base item as a goal makes the game
panic. The scan is cached per (collection base, name), and the cache is keyed by collection so that
parallel background sims with different collections do not thrash it.

For vanilla items this is all unnecessary: `id == catalog index`, so the id is used directly.

---

## 5. Telling *enabled* mods from merely *loaded* ones

The game loads item definitions from disabled mods too — they end up in the same array, flagged as
inactive, and the in-game codex filters them out. A mod that scans that array naively will happily
offer you items from mods you turned off, and designating one silently does nothing.

**The reliable test is a field on the entry itself:**

```
ModItemEntry + 0x190   != 0  → active
                       == 0  → inactive
```

Evidence for that offset (this is worth knowing because guessing here is expensive):

* the game's **own `Debug` impl** (`0x21a0c10`) branches on this exact field to render
  `ModItemEntry(<id>, active)` vs `…, inactive)` — `cmp qword [rcx+0x190], 0 / sete / cmove`;
* independently, `0x1408f0870` loops over the array and processes **only** entries whose
  `[rsi+0x190] == 0`.

`mod_final_opts()` filters the dropdown to active entries only, mirroring the game's own criterion.

Two rules that came out of getting this wrong:

* **Do not use `mods.json` + per-mod `text/item.i18n` cross-referencing.** That was the original
  approach (`enabled_mods()` still exists for diagnostics). It was accurate enough until 0.5.2, when
  inactive entries started arriving in the same `Vec`; and it fails for enabled mods that have no
  `item.i18n` at all. Game memory is the source of truth.
* **"Zero active items" is a valid state, not a detection failure.** The first version of the filter
  assumed that everything-inactive meant a misdetection and disabled the filter — which restored the
  exact bug it was meant to fix, because a profile with no item mods enabled legitimately has zero.
  The only fail-safe left is "flags not collected yet (before the scan)", and an entry whose flag
  could not be *read* defaults to **active** — dropping an item you cannot read about would silently
  erase a user's saved designation, and the 7 vanilla categories always remain in the list anyway.

---

## 6. In-match display: reading the view model

The game draws item icons by walking a **hardcoded array of three node names** (`"slotN"`); the
`cmp rbx, 0x30` in that loop is the *byte size of that name array*, not an item limit. The item
iteration next to it is a normal `i < items.len()` — so `items[3]` exists in the data, it simply is
never visited.

Two approaches were tried and abandoned:

* **Extending the game's loop** (raising the bound, relocating the name array, extending the stack
  frame). All 84 patch sites validated, and the game still froze on match entry. Sealed behind
  `SLOT_UI_SURGERY = false`; the failure is documented in the source above `patch_slot_ui`.
* **Caching champion → icon from the buy hook.** Structurally impossible to keep clean: your athletes
  exist simultaneously in the on-screen match and in background pre-sims, so a build finished in the
  background leaks onto the on-screen player. (It also assumed one `blue_player` node when there is
  one per lane — the real source of the "wrong item" reports.)

What ships reads **the same data the game reads**, with no patching at all:

```
GameView  = App + 0x4a50               (captured once from game.rs update, rcx)
  item_list   +0xa8 cap / +0xb0 ptr / +0xb8 len     element 16B = (data, vtable)
  player_view +0x1d0 ctrl / +0x1d8 mask / +0x1e8 count      hashbrown RawTable
      entry stride 0x260, entries run BACKWARDS from ctrl:  E = ctrl − (i+1)*0x260
      key    +0x00 team (0 = blue, 1 = red) / +0x08 position (0 top … 4 support)
      items  +0x50 cap / +0x58 ptr / +0x60 len      Vec<u64> of item_list indices
```

No hashing is needed — with at most 10 entries a linear bucket scan is simpler and safe. Icons are
distinguished by the ImageRunner's **`rect_tag`** (`+0x18`), not by `source` (`+0x0`), which is a
fixed spritesheet path.

The tooltip is the same idea taken one step further: the mod does not draw a tooltip, it **calls the
game's own tooltip `show` function** (`0x1ab52f0`, 11 arguments) on the game's own `#item_tooltip`
node. Name, tier, price, all 24 stats, effect text, i18n, sizing, positioning and screen clamping are
therefore handled by the game — which is why **mod items get a correct tooltip for free**. The
arguments (`p1 = arg5`, `p2 = arg6`, `root = arg4` of the UI mega-function) are captured from an
existing detour; getting them off by one position dereferenced a UI node as a registry and killed the
process instantly.

Because the game uses that same node for slots 0–2, ownership is explicit: if the game is hovering
one of its own slots this frame, the mod does not touch the node; the mod only lowers tooltips it
raised itself.

---

## 7. UI injection: why there is no asset override

`mod.override_info` in this mod is `{}` — deliberately empty. The 4 dropdowns, the extra column
header and the 4th in-match slot are all injected by **hooking the asset loader** instead.

### 7.1 What the override system would do

The game supports two kinds of asset intervention:

| mode | semantics | applies to `.ui`? |
|---|---|---|
| `merge` | recursive JSON merge of raw bytes (arrays are replaced wholesale) | **no** — `.ui` does not keep raw bytes, so merge is unavailable for it |
| `override` | alias remapping: the target key resolves to your file instead | yes |

So for a `.ui` file, `override` is the only option, and it is **whole-file replacement**. That has
three consequences this mod cannot live with:

1. **It does not compose.** If two mods override the same `.ui`, one wins and the other's changes
   vanish. `strategy.ui` and `training.ui` are exactly the files other tactics/comp-test mods want to
   touch. (A real incident: two mods overriding `champion_info` produced a screen that simply
   crashed.)
2. **It rots on every game patch.** An override file is *base + your delta*, so the moment the game
   ships a new base the override silently reverts the new base to the old one. In 0.5.3 that turned
   `champion_info.ui` (1,248 → 2,047 lines, node types changed) into a hard crash on screen entry for
   a mod that had not re-based. Every patch would mean re-basing `strategy.ui` (86 KB) and both
   `player_info` variants by hand.
3. **It is all-or-nothing per file.** We want to *add four dropdowns to existing rows*, not to own
   the file.

### 7.2 What the mod does instead

`ui_inject.rs` chain-hooks the engine's asset-get function (`LOADER_RVA = 0x2e1550`) and post-processes
the template **after** the game (and any override, from any mod) has produced it:

```rust
extern "win64" fn detour(am, path, len) -> usize {
    let r = original(am, path, len);   // whatever the game/other mods produced
    loader_body(path, len, r);         // ...then we edit the result
    r
}
```

`loader_body` matches on the asset path and does one of two things:

* `…/ingame_component/player_info` and `…/wide_player_info` → **replace the root's children** with
  our parsed `.ui` (these files are ours alone; nobody else touches them);
* `…/layout/strategy` and `…/layout/training` → **append** our nodes to existing containers
  (`row0..row4`, `personal_header`, `blue0..4` / `red0..4`).

Appending is the important half:

> **Never call `replace_children` on a shared container.** It deletes nodes appended by other mods.
> The comp-test rows are shared with `comptest_unlock`; replacing them wiped its nodes.

Two more details that make this work in practice:

* Every injected node id carries a mod prefix (`it4_s0`, `it4_slot3`, `item0m`, …) so it cannot
  collide with a native id — the native tree already contains ten `#item3:image` nodes.
* Injection is **idempotent**: presence of the marker node (`it4_s0`, or `item3` / `item2m` on the
  strategy screen) means "already done", and the last-seen template pointer is remembered so a
  reloaded template is re-processed but the same one is not.

Note what is *not* being claimed: this only makes **additive** edits composable. Two mods that
*rewrite* the same subtree still conflict; `player_info` is replaced wholesale only because it is
verified that no other mod touches it.

### 7.3 Why the native dropdowns could not be reused

The native `item0/1/2` dropdowns reject any option index ≥ 7: a click commits into the vanilla
`personal_tactics` model, which only knows the 7 built-in categories, and the value never reaches the
runner. (Confirmed twice by RE — including NOPing the only `+0x1788` revert-writer in the binary,
which changed nothing.) So the mod overlays its own dropdowns (`item0m/1m/2m` + `item3`) on top,
because an appended child renders above and receives the hit first, and hides the natives with
`visible = false`. A mod-owned dropdown commits directly to `runner + 0x1788`, which the mod then
polls.

### 7.4 If you *did* want to use override

It is a reasonable choice for a mod that owns a screen. To convert this one you would:

1. Move the injected fragments into full `.ui` files built as **base + delta** and list them in
   `mod.override_info` as `{"remapping": …, "type": "override"}` — one entry per target
   (`strategy`, `training`, `player_info`, `wide_player_info`).
2. Keep `ui_inject.rs` only for the parts that must survive alongside other mods, or drop it and
   accept that your mod owns those screens exclusively.
3. Add a **patch-time re-base step**: for every override target, diff the old and new base file
   (normalise line endings first — a raw CRLF diff flags every file) and re-apply the delta to the
   new base whenever the base changed. Validate the result with "zero deleted lines vs the new base",
   balanced braces, no BOM, and that every i18n key exists for every locale.
4. Expect to lose composability with any other mod that targets the same files, and to have to
   coordinate manually with them.

The runtime cost of the hook approach is one string comparison per asset load. That is why it won.

---

## 8. Coexisting with other mods

* **Chain hooking.** When another mod has already hooked a shared function, its entry point holds
  `48 b8 <target> ff e0` (`movabs rax, target; jmp rax`) instead of the original prologue. Installing
  over that would orphan the other mod's hook. So `install_detour_generic` detects that pattern and
  builds a trampoline that jumps to **the foreign stub** rather than executing an original prologue
  that is no longer there — both detours then fire in sequence. Prologue validation is skipped in that
  case, because the prologue legitimately is not the original any more.
* **Install late, once.** The launcher hook is installed from `post_update` (after other mods' init),
  waits up to 240 frames for a foreign hook to appear, and only then installs standalone. Installing
  first would just get overwritten.
* **Never re-chain every frame.** Two mods that both re-validate and re-chain their entry point each
  frame will chain to each other in a cycle and hang the game. (This actually happened between
  `draft_overlay` and this mod.) Re-validation here runs every 60 frames and only re-chains when the
  entry point is neither the original nor our own stub.
* **Version gating must not inspect shared hooks.** See below — this one bit us.

---

## 9. Version gate

`check_game_version()` runs at the very top of `init()` and, on a mismatch, returns a bare
`ModRegistration` — **no hook, no patch, nothing**. Two independent checks:

1. exe file size (0.5.3 = 74,970,624 B);
2. the entry prologues of `buy` and `seedctor`.

The subtle rule: **only functions that no other mod hooks may be used as version evidence.** The
launcher was originally in that list, and because another mod (`serpen`) legitimately chain-hooks it,
its entry point is *supposed* to be a `movabs+jmp` — which the gate read as "wrong version" and
disabled the entire mod on the correct version. Now an already-hooked entry point counts as a pass,
and only the mod's private functions are byte-compared.

There is a second gate in `mod.mod_info`: `"dependencies": [{"mod_id": "base", "version": ">=0.5.3,<0.5.4"}]`,
so the loader will not even attach the mod on 0.5.4.

---

## 10. Safety rules used throughout

These are not optional in a native mod that runs inside a parallel simulation:

* Every detour body is wrapped in `catch_unwind(AssertUnwindSafe(…))` — a panic unwinding into the
  game's call stack is undefined behaviour. Exception: the launcher detour, which runs on a 91 KB
  `chkstk` frame and therefore contains no panic source at all (raw reads and atomics only).
* All raw reads go through `safe_read_*`, backed by a **vectored exception handler** that turns an
  access violation into a `None`. The handler itself must never allocate, lock, format or panic; its
  state is a `Cell` array in TLS with `const` initialisation and no `Drop`, so there is no lazy-init
  flag and no TLS destructor to trip over.
* Mutex use in detour-reachable code is poison-safe (`.lock().unwrap_or_else(|e| e.into_inner())`).
* Data shared with parallel detours (the designation snapshot, the comp-test roster) is published as
  an immutable `AtomicPtr` snapshot and **never freed** — a detour on another thread may be reading
  it. Rebuilds are guarded by a content signature, so an unchanged frame leaks nothing.
* Calling a game function from the mod ("shadow-call") is validated first: the pointer must point at
  a `PAGE_EXECUTE_*` page, and structure signatures are re-checked **on every call**, not once at
  detection — pointers inside the game's own structures go stale across session transitions.

---

## 11. Performance notes

Everything below came out of `perf.rs` (an rdtsc-based per-site profiler that compiles away entirely
when `PERF_ON = false`). They are recorded because each one was a *surprise*:

* **`readable()` is `VirtualQuery`, i.e. a kernel call.** One of those at the top of the buy detour
  cost 75 % of the mod's entire budget (6.89 M calls in 130.7 s). Moving every validity check behind
  the cheap early-exit tests reduced it to 1.2 %.
* **A global spinlock in the VEH read helper serialised every rayon worker.** Replacing the shared
  `SEH[8]` state with thread-local state removed the contention entirely — the handler runs on the
  faulting thread, so its own TLS *is* the right state.
* **"Lightweight, ~52 entries" was wrong.** Rebuilding the delegate snapshot every frame cost 174 µs
  per in-game frame. It only changes on user action, so it is throttled to every 20 frames.
* **Re-installing hooks every frame is not free** even when it early-returns; the self-heal
  re-validation is throttled to 1 Hz.
* Measure the probe itself. Each site's number includes one probe's worth, and the report prints it
  so it can be subtracted.

---

## 12. Building

```
rustup run nightly-2026-05-24 rustc \
  --crate-type cdylib --edition 2021 \
  -C opt-level=1 -C overflow-checks=off \
  -C linker-flavor=lld-link -C linker=rust-lld \
  -L dependency=<sdk>/deps -L native=<sdk>/native \
  --extern mod_api=<sdk>/deps/libmod_api-*.rlib \
  --extern engine_ui=<sdk>/deps/libengine_ui-*.rlib \
  --extern engine_core=<sdk>/deps/libengine_core-*.rlib \
  src/lib.rs -o tfm2_item_tactics.dll
```

* The toolchain must match the SDK rlibs exactly (`nightly-2026-05-24` for the 0.5.3 SDK).
* **`opt-level=1`, not 2 or 3.** Higher levels inflate the reproduced detour frames enough to hit
  `STATUS_STACK_OVERFLOW` where a rayon worker's stack meets the game's recursive sim.
* `rust-lld` rather than MSVC `link.exe`, which fails with `LNK1107` on the 0.5.3 SDK rlibs.
* Deploy `tfm2_item_tactics.dll`, `mod.mod_info`, `mod.override_info`, `4items.cfg` and
  `ui/layout/strategy.ui` into `<game>/mods/tfm2_item_tactics/`.
* `mod.mod_info` must be **UTF-8 without BOM** — a BOM makes the loader's parser fail and the mod is
  silently disabled. (Check: the first byte is `0x7b`, `{`.)

`src/lib.rs` pulls in `src/ui_inject.rs` and `src/perf.rs` via `#[path]`, and embeds
`ui/layout/ingame_component/*.ui` with `include_str!`, so the directory layout in this archive is the
layout the compiler expects.

## 13. Configuration

`4items.cfg`, next to the dll:

```
slots = 4   →  4 item slots; the 4th is designated or auto-recommended
slots = 3   →  3 item slots; all 4-item behaviour off (vanilla purchasing, designation still works)
```

A restart is required after changing it. Loading always leaves a trace file, because a silent
fallback to the default made a mistyped path indistinguishable from a broken feature.
