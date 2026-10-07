Adds items inspired from Riot Games (LoL, TFT) to Teamfight Manager 2.  
Reskins the 30 existing items and also adds 235 new items (137 base + 98 Radiant) to the game.

##### Instructions  
If you are only seeing Bloodthirster/Luden's/Sunfire (vanilla items, no modded), that means the save you are playing is not loading the mod order. To fix this, try the following:

1. Save your current game and go back to the main menu.
2. Click Load -> Load on the save you just made. It will probably say "No Info" under mods.
2. Once launched, progress once and save again. Then go back to the main menu.
3. Click Load -> Load again, this time the mods column should read "Match". <-- (That means it's loading correctly)
4. Play as you would normally!

##### Check it out on Steam Workshop! 
https://steamcommunity.com/sharedfiles/filedetails/?id=3739568852
##### Item Scroller Mod Add-On
https://steamcommunity.com/sharedfiles/filedetails/?id=3739984076  
https://github.com/shirograhm/item-scroller-tfm2/releases

##### Gold Efficiency Stats - For Base Values
https://claude.ai/artifact/Sqb2fwDxZ8EYtipyXsGrYt

##### Item Info Screen
<img width="1748" height="1133" alt="image" src="https://github.com/user-attachments/assets/b19e829d-15d2-4010-aa7d-1839138eb8e5" />

##### Pregame Item Builds Editor
<img width="2049" height="1183" alt="item build editor" src="https://github.com/user-attachments/assets/27bfd055-4eea-49c2-8736-484c56c3afe6" />


##### Smart Builds
The Build Editor footer has a toggle, **Enforce Smart Builds**, on by default. It cleans up what the AI picks. Any of these is swapped for another final item of the same category:
- a duplicate item
- a second Grievous Wounds item
- a second Spellblade item (Trinity Force, Dusk and Dawn, Lich Bane, Essence Reaver, Iceborn Gauntlet or Bloodsong). Sheen doesn't count, since it's the component they build from. A support whose World Atlas item is Bloodsong keeps that one and no other. If you pinned a Spellblade item yourself, the AI adds none (your own pins can hold more than one)
- a crit item that would push the build past 100% crit chance (counting crit from item passives as fully stacked)
- a jungle item (Feral Flare, Grez's Spectral Lantern or Philosopher's Stone) on any champion not playing the jungle role, and on a jungler any jungle item other than its one (see below)
- a melee item on a ranged champion (Ravenous Hydra, Titanic Hydra, Hullbreaker, Heartsteel, Abyssal Mask, Sunfire Cape, Hollow Radiance, Philosopher's Stone), or a ranged item on a melee champion (Runaan's Hurricane, Diamond Tipped Spear). A champion is melee when its basic attack range is 35 or less; champions from other mods are covered too
- a support item (except Protoplasm Harness and Zeke's Convergence) on any champion not playing the support role
- an item that only works when its carrier heals, shields or buffs an ally (Ardent Censer, Echoes of Helia, Moonstone Renewer, Staff of Flowing Water) on a support whose kit does none of that, or on a mage (see below). What counts is the champion's basic abilities, not the game's Heal and Shield tags: a Vampire only heals itself, and a Bard only buffs
- an item the champion doesn't scale with: attack, attack speed or crit on an AP champion, or ability power on an AD champion. That includes Hextech Gunblade, the one hybrid item (attack damage together with ability power), which only hybrid champions keep. Hybrid champions are left alone, and so are the four heal, shield and buff items above, which are for what the champion casts. Magic Knight counts as an AP champion, since only its basic attack is physical. Dusk and Dawn, Lich Bane and Nashor's Tooth count as AP items despite their attack speed, and so does Rite of Ruin, whose crit only feeds its own shield
- Guinsoo's Rageblade, Statikk Shiv or Sword of Blossoming Dawn on any champion that isn't a marksman (the game's Range class). Marksmen keep them whatever their damage type. Sword of Blossoming Dawn is a support item, so it is for a marksman playing support

For the last three, the replacement follows the rest of the build instead of the removed item: it comes from the same category as the build's other items, starting with the first slot.

Every support also gets a World Atlas item as its first pick, so it starts the match on World Atlas and its gold: World Atlas grows into Runic Compass and then one of five finished items. A mage (the game's Magician class) gets Zaz'Zak's Realmspike, since its abilities are there to deal damage, even when its kit also stuns, roots or shields. Otherwise, a support whose kit heals, shields or buffs allies gets Dream Maker, since its casts on allies are what set it off. Otherwise, a support with the game's Tank tag gets Celestial Opposition, since it is the one being hit. Otherwise, a support whose kit immobilizes (the same champions that take Imperial Mandate) gets Solstice Sleigh, and any other AP support gets Zaz'Zak's Realmspike too. Every other support gets Bloodsong, whatever its damage type. It replaces the AI's last pick, unless you pinned one yourself. The AI only ever gets that one: any other it picked is swapped out (your own pins can hold more than one).

Junglers also get an item for their role: Philosopher's Stone for a champion with the game's Tank tag, otherwise Feral Flare or Grez's Spectral Lantern (whichever suits the champion's damage type). A ranged tank takes its damage item too, since Philosopher's Stone is a melee item. The AI only ever gets that one jungle item: any other it picked is swapped out, and if you pinned a jungle item yourself it adds none (your own pins can hold as many as you like). If the AI didn't pick one and you haven't pinned one, its last pick is swapped for one. Only one is guaranteed; the rest of the build is still the AI's own choice.

A support's World Atlas item is its one guaranteed support item. After it, supports prefer support items but aren't held to them, and can build anything that suits their class: the AI leans toward support items when it picks (ones that suit the champion's damage type, so an AD support leans toward the tank ones, not the AP ones), buys any it picked right after the World Atlas item, and looks at support items first when it has to replace one of its picks. Mages (the game's Magician class) don't lean toward support items: after their Zaz'Zak's Realmspike they build AP items like any other mage. If the AI did pick a support item, a support whose kit immobilizes (a stun, root, knock-up, knockback, pull, taunt, fear or charm; a slow, silence or disarm doesn't count) makes it Imperial Mandate, unless you pinned a support item yourself; Mandate is an AP item, so an AD support keeps its own. A support that doesn't immobilize never keeps a Mandate the AI picked: it's swapped for another support item (a Mandate you pin is always kept). A support whose kit heals, shields or buffs allies makes another support item the AI picked one of the four items that need that (Ardent Censer, Echoes of Helia, Moonstone Renewer, Staff of Flowing Water), whatever its damage type, unless you pinned a support item yourself. Neither adds a support item the AI didn't pick. Mages never keep one of those four, even when their kit shields: it's swapped for an item matching the rest of their build.

It also decides the order the AI buys its picks in. A support's World Atlas item comes first of all, then a support's support items (not a mage's) or a jungler's jungle item. After that, items that get stronger the longer you own them are bought first: Heartsteel, Yun Tal Wildarrows, Hubris, Feral Flare, Grez's Spectral Lantern, Collector and Rod of Ages. Items that scale off stats from the rest of the build are bought last: Riftmaker, Overlord's Bloodmail, Atma's Reckoning, Protector's Vow, Cloak of Starry Night, Rabadon's Deathcap, Deathblade, Infinity Edge and Lord Dominik's Regards. Everything else keeps the AI's order.

It covers all six slots, but only ever changes items the AI picked: an item you pin in the editor is always kept exactly as set, in the slot you put it in, and the AI's picks around it make way for it (for example, the AI won't also build an item you pinned elsewhere in the build). Support items get no pass on the damage-type check: an AD support won't keep an AP support item like Imperial Mandate, though tank support items are fine (and so are the four heal, shield and buff items). Sword of Blossoming Dawn counts as an AP item here, despite its attack speed. Switch it to **Allow Any Builds** to leave every pick alone.

It also gives every AI build a pair of boots, as its second pick, unless you pinned boots yourself: Berserker's Greaves for marksmen, Sorcerer's Shoes for mages, Plated Steelcaps or Mercury's Treads for tanks (whichever answers the enemy's main damage type), Gluttonous Greaves for fighters and Ionian Boots of Lucidity for assassins. Supports get Ionian Boots of Lucidity too, unless they're tanks, who get tank boots. Boots of Swiftness goes to champions none of that fits. Boots are also in the Build Editor under their own **Boots** group.

##### In-Game Chat Fix
The champion info tooltip draws on top of the in-match chat instead of underneath it. This is the [Ingame Chat Foreground Fix](https://steamcommunity.com/sharedfiles/filedetails/?id=3743712915) mod, built in. If you have the standalone mod, disable it: both replace the same in-match layout, so only one can apply, and if the standalone one wins, the wide scoreboard goes back to the base game's width, which six item slots don't fit.

### Added Items

Ability Haste works like League's: cooldown = base × 100 / (100 + Ability Haste), so 20 Ability Haste is a 16.7% shorter cooldown. It applies to every skill, ultimate included; Ultimate Ability Haste stacks on top for the ultimate only.

#### Tier 1
| Item | Cost | Stats | Passive |
| --- | --- | --- | --- |
| **Boots** | 250G | +7% MS | — |
| **Glowing Mote** | 250G | +10 Ability Haste | — |
| **World Atlas** | 250G | +3 HP Regen | Shared Riches: Gain 2 gold every 5 seconds. |

#### Tier 2
| Item | Cost | Stats | Passive |
| --- | --- | --- | --- |
| **Fated Ashes** | 250G | +30 AP | Inflame: Dealing Ability damage burns enemies for 15 magic damage over 3 seconds. This effect is 400% as effective against minions and monsters. |
| **Runic Compass** | 450G | +150 HP<br>+3 HP Regen | Shared Riches: Gain 3 gold every 5 seconds. |

#### Tier 3
| Item | Cost | Stats | Passive |
| --- | --- | --- | --- |
| **Aegis of the Legion** | 700G | +100 HP<br>+20 Armor<br>+30 MR | — |
| **Bami's Cinder** | 700G | +200 HP | Immolate: Deal 5 + 0.5% of your maximum health as magic damage to all enemies within 30 range. This effect deals 50% more damage to minions and monsters. |
| **Bandleglass Mirror** | 350G | +100 HP<br>+1 HP Regen<br>+10 AP<br>+5 Ability Haste | — |
| **Berserker's Greaves** | 650G | +15% AS<br>+10% MS | — |
| **B.F. Sword** | 450G | +35 Attack Damage | — |
| **Blighting Jewel** | 400G | +40 AP<br>+10% Magic Pen | — |
| **Boots of Swiftness** | 650G | +25% Tenacity<br>+12% MS | — |
| **Catalyst of Aeons** | 500G | +100 HP<br>+20 AP<br>+5 Ability Haste | Eternity: Heal for 15 - 40 (based on level) health per Ability cast. |
| **Caulfield's Warhammer** | 500G | +25 AD<br>+10 Ability Haste | — |
| **Executioner's Calling** | 650G | +25 Attack Damage | Grievous Wounds: Dealing physical damage to an enemy champion reduces their healing by 25% for 2 seconds. |
| **Forbidden Idol** | 350G | +100 HP<br>+10 AP<br>+10 Ability Haste | — |
| **Glacial Buckler** | 400G | +25 Armor<br>+5 Ability Haste | — |
| **Gluttonous Greaves** | 650G | +8% Omnivamp<br>+10% MS | — |
| **Haunting Guise** | 500G | +100 HP<br>+30 AP | Madness: For each second in combat with enemy champions, deal 2% bonus damage, stacking up to 3 times for a total of 6%. Stacks are lost after 3 seconds out of combat. |
| **Hearthbound Axe** | 500G | +20 AD<br>+20% AS | — |
| **Hextech Alternator** | 400G | +50 AP | Revved: Damaging an enemy champion deals 65 bonus magic damage (40 second cooldown). |
| **Ionian Boots of Lucidity** | 650G | +15 Ability Haste<br>+10% MS | — |
| **Last Whisper** | 500G | +25 AD<br>+10% Armor Pen | — |
| **Mercury's Treads** | 650G | +20 MR<br>+20% Tenacity<br>+10% MS | — |
| **Needlessly Large Rod** | 450G | +60 Ability Power | — |
| **Noonquiver** | 400G | +25 AD<br>+10% Crit Chance | — |
| **Oblivion Orb** | 650G | +45 Ability Power | Grievous Wounds: Dealing magic damage to an enemy champion reduces their healing by 25% for 2 seconds. |
| **Phage** | 500G | +100 HP<br>+20 AD | Rage: Basic attacks against enemy champions grant 5% movement speed for 2 seconds. |
| **Plated Steelcaps** | 650G | +15 Armor<br>+10% MS | Plating: Reduce damage taken from basic attacks by 5%. |
| **Scout's Slingshot** | 400G | +30% AS | Bullseye: Damaging an enemy champion deals 40 bonus magic damage (20 second cooldown). |
| **Seeker's Armguard** | 500G | +30 AP<br>+20 Armor | Witch's Path: Killing a unit grants 0.5 armor, up to a maximum of 15. |
| **Serrated Dirk** | 400G | +25 AD | Gain 10 Lethality. |
| **Sheen** | 650G | +20% AS<br>+10 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 30 - 85 (based on level) bonus physical damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Sorcerer's Shoes** | 650G | +15% Magic Pen<br>+10% MS | — |
| **Spirit Stone** | 400G | +100 HP<br>+20 AP | Butcher: Against monsters, deal 10% bonus magic damage and restore health equal to 2% of your damage dealt. |
| **Steel Sigil** | 500G | +20 AD<br>+20 Armor | — |
| **Tiamat** | 400G | +25 AD | Cleave: Basic attacks deal 20% of your Attack Damage as physical damage on-hit to nearby enemies. Attacks from further than 35 range apply this effect at 50% strength. |
| **Verdant Barrier** | 400G | +35 AP<br>+25 MR | Annul: Grants a spell shield that blocks the next enemy Ability (40 second cooldown). |
| **Winged Moonplate** | 400G | +150 HP<br>+4% MS | — |

#### Tier 4
| Item | Cost | Stats | Passive |
| --- | --- | --- | --- |
| **Abyssal Mask** | 700G | +150 HP<br>+50 MR<br>+10 Ability Haste | Unmake: Reduce the magic resistance of enemy champions within 50 range by 20%. |
| **Ardent Censer** | 500G | +100 HP<br>+1 HP Regen<br>+25 AP<br>+5 Ability Haste<br>+5% MS | Sanctify: Healing, shielding or buffing an allied champion (excluding yourself) grants them 20% attack speed and bonus physical damage on-hit equal to 2% of the target's maximum health for 6 seconds. |
| **Atma's Reckoning** | 750G | +250 HP<br>+20% Crit Chance | Big Hands: Gain 5% critical strike chance for every 1000 maximum health, up to 25%. |
| **Axiom Arc** | 650G | +35 AD<br>+10 Ability Haste | Gain 18 Lethality.<br>Flux: Gain 10 (+0.2 per 1 Lethality) Ultimate Ability Haste. |
| **Bandlepipes** | 500G | +100 HP<br>+20 Armor<br>+30 MR<br>+15 Ability Haste | Fanfare: Dealing damage to an enemy champion empowers you with Fanfare for 4 seconds, granting you 12% bonus movement speed. While empowered, you and allied champions within 100 range also gain 20% attack speed. |
| **Banshee's Veil** | 700G | +60 AP<br>+40 MR | Annul: Grants a spell shield that blocks the next enemy Ability (40 second cooldown). |
| **Bastionbreaker** | 650G | +35 AD<br>+15 Ability Haste | Gain 22 Lethality.<br>Sabotage: Scoring a takedown on an enemy champion grants Sabotage for 90 seconds, empowering your next basic attack against a turret to deal 150 + 15% of your Attack Damage as bonus true damage. |
| **Black Cleaver** | 750G | +25 AD<br>+150 HP<br>+5 Ability Haste | Carve: Dealing physical damage to enemy champions reduces their armor by 6% for 6 seconds (max 5 stacks). |
| **Blackfire Torch** | 650G | +65 AP<br>+15 Ability Haste | Maleficent: Landing an Ability on an enemy champion grants 5 Ability Power for 4 seconds (max 4 stacks). |
| **Blade of the Ruined King** | 750G | +25 AD<br>+25% AS<br>+5% Omnivamp | Mist's Edge: Basic attacks deal bonus physical damage on-hit equal to 5% of the target's current health. Deals a maximum of 50 physical damage against minions and monsters. |
| **Bloodletter's Curse** | 750G | +150 HP<br>+55 AP<br>+5 Ability Haste | Decay: Dealing magic damage to enemy champions reduces their magic resistance by 6% for 6 seconds (max 5 stacks). |
| **Bloodsong** | 550G | +200 HP<br>+4 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 70 - 125 (based on level) bonus magic damage on-hit (1.5 second cooldown, starting after using the empowered attack). If the target is a champion, increase their damage taken by 7% for 4 seconds. |
| **Celestial Opposition** | 550G | +200 HP<br>+4 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Blessing of the Mountain: Become Blessed upon taking damage from an enemy champion, reducing your damage taken by 15% for 2 seconds. After Blessed ends, unleash a shockwave around you that slows enemies nearby by 50% for 1.5 seconds (18 second cooldown, beginning after the explosion). |
| **Chempunk Chainsword** | 650G | +20 AD<br>+200 HP<br>+10 Ability Haste | Grievous Wounds: Dealing physical damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Chemtech Putrifier** | 500G | +150 HP<br>+2 HP Regen<br>+15 AP<br>+15 Ability Haste | Grievous Wounds: Dealing damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Cloak of Starry Night** | 750G | +200 HP<br>+50 MR<br>+25% Total MR | Limitless as the Stars: Increase your total magic resistance by 25%. Additionally, gain 5% (+1% per 25 magic resistance) skill damage reduction, up to a maximum of 25%. |
| **Collector** | 750G | +35 AD<br>+20% Crit Chance | Gain 10 Lethality.<br>Death: Dealing damage to enemy champions below 6% maximum health executes them.<br>Taxes: Killing a champion grants you an additional 25 gold. |
| **Cosmic Drive** | 850G | +150 HP<br>+50 AP<br>+10 Ability Haste<br>+4% MS | Spelldance: Dealing magic damage or true damage to an enemy champion grants you 8% movement speed for 4 seconds. |
| **Dead Man's Plate** | 750G | +150 HP<br>+30 Armor<br>+4% MS | Shipwrecker: While moving, generate 7 stacks of Momentum every second, up to 100. Each stack grants 0.25% bonus movement speed. Basic attacks consume all remaining Momentum to deal 0 - 200 bonus physical damage on-hit, scaling with stacks consumed. |
| **Deathblade** | 700G | +50 AD | Apex: Increase your total Attack Damage by 15%. |
| **Death's Dance** | 750G | +30 AD<br>+30 Armor<br>+10 Ability Haste | Ignore Pain: 25% of the damage you take is dealt over time as true damage (up to 2.5% of your maximum health per second).<br>Defy: Scoring a takedown on an enemy champion cleanses the remaining stored damage and heals you for 45 + 15% of your missing health. |
| **Diamond Tipped Spear** | 750G | +35% AS<br>+10 Ability Haste | Pierce: Gain 30 Adaptive Force. Each Adaptive Force grants 0.6 Attack Damage or 1 Ability Power, depending on which is higher.<br>Sweet Spot: Deal up to 25% bonus damage to enemy champions based on distance (maximum effect at 100 range). |
| **Dream Maker** | 550G | +200 HP<br>+4 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Dream Maker: Healing, shielding, or buffing an allied champion grants them a Dream Bubble for 5 seconds. While the Dream Bubble is active, it grants 8% skill damage reduction and 20 Adaptive Force (8 second cooldown, starting after the Dream Bubble ends). |
| **Dusk and Dawn** | 700G | +100 HP<br>+30 AP<br>+25% AS<br>+10 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 85 + 15% of your Ability Power as bonus magic damage on-hit and heal you for 10% of your Ability Power and 2.5% of your maximum health (1.5 second cooldown, starting after using the empowered attack). |
| **Echoes of Helia** | 550G | +150 HP<br>+2 HP Regen<br>+25 AP<br>+15 Ability Haste | Soul Siphon: Store 30% of the damage you deal or take as Soul Charges, up to 130 - 350 (based on level). Healing, shielding or buffing an allied champion (excluding yourself) consumes all Soul Charges and heals them equal to the consumed amount. |
| **Eclipse** | 650G | +40 AD<br>+15 Ability Haste | Ever Rising Moon: Landing a basic attack or an Ability on an enemy champion marks them for 2 seconds, up to once per cast instance. Hitting a marked champion consumes the mark to deal bonus physical damage equal to 5% of their maximum health and grant you a shield that absorbs 100 + 15% of your Attack Damage for 2 seconds (6 second cooldown per target). |
| **Edge of Night** | 750G | +35 AD<br>+100 HP | Gain 15 Lethality.<br>Annul: Grants a spell shield that blocks the next enemy Ability (40 second cooldown). |
| **Endless Hunger** | 700G | +40 AD<br>+5% Omnivamp<br>+20% Tenacity | Famine: Gain 5 (+5% AD) Ability Haste.<br>Feast: Scoring a takedown on an enemy champion grants 15% Omnivamp for 5 seconds. |
| **Essence Reaver** | 800G | +30 AD<br>+20% AS<br>+10 Ability Haste<br>+20% Crit Chance | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 125% of your Attack Damage (+100% Crit Chance) as bonus physical damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Experimental Hexplate** | 600G | +150 HP<br>+30% AS | Hexcharged: Gain 15 Ultimate Ability Haste.<br>Overdrive: Upon casting your ultimate ability, enter Overdrive, gaining 32% attack speed and 16% movement speed for 8 seconds (32 second cooldown, starting on cast). |
| **Feral Flare** | 700G | +25 AD<br>+20% AS<br>+10 Armor | Maim: Gain a Feral stack for each champion takedown scored and monster killed, up to 50. Basic attacks deal 25 (+1 per Feral stack) bonus magic damage and restore 10 health. This effect is 150% as effective against minions and monsters. |
| **Fiendhunter Bolts** | 700G | +35% AS<br>+20% Crit Chance<br>+4% MS | Night Vigil: Gain 15 Ultimate Ability Haste.<br>Opening Barrage: Upon casting your ultimate ability, gain 50% attack speed for 8 seconds. Your next 3 basic attacks within this window critically strike for 60% bonus damage. If the attack would have already critically struck, it instead deals bonus true damage equal to 15% of its damage dealt (45 second cooldown). |
| **Frozen Heart** | 650G | +35 Armor<br>+10 Ability Haste<br>+10% Skill DMG Reduction | Winter's Caress: Reduce the attack speed of enemy champions within 100 range by 30%. |
| **Frozen Mallet** | 700G | +200 HP<br>+20 AD | Icy: Basic attacks apply a 15% slow for 2 seconds. |
| **Grez's Spectral Lantern** | 700G | +150 HP<br>+30 AP<br>+10 Ability Haste | Spirit Drain: Gain 1 Ability Power for each champion takedown and monster killed, up to 20.<br>Butcher: Against monsters, deal 20% bonus magic damage and restore health equal to 4% of your damage dealt. |
| **Guardian Angel** | 750G | +35 AD<br>+30 Armor | Rebirth: Upon taking lethal damage, instead resurrect for 4 seconds, healing for 40% of your maximum health. While resurrecting, you are untargetable, invulnerable, and unable to act (300 second cooldown). |
| **Guinsoo's Rageblade** | 700G | +15 AD<br>+15 AP<br>+30% AS | Wrath: Basic attacks deal 30 bonus magic damage on-hit.<br>Seething Strike: Basic attacks grant 8% attack speed for 4 seconds (max 4 stacks). |
| **Hamstringer** | 750G | +25 AD<br>+25% AS<br>+20% Crit Chance | Scour: Your critical strikes bleed the target, dealing 55 - 110 (based on level) (+100% Crit Chance) physical damage over 2 seconds and applying a 7% slow. |
| **Heartsteel** | 750G | +250 HP | Ironheart: If an enemy champion stays within 50 range of you for 1.5 seconds, your next basic attack against them deals bonus physical damage on-hit equal to 70 + 6% of your maximum health, granting 10% of that damage as permanent bonus health (30 second cooldown per target).<br>Goliath: Gain 3% increased size for every 1000 maximum health, up to 30%. |
| **Hextech Gunblade** | 750G | +25 AD<br>+50 AP<br>+10% Omnivamp | — |
| **Hollow Radiance** | 750G | +200 HP<br>+50 MR<br>+4% Skill DMG Reduction | Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies within 30 range. This effect deals 50% more damage to minions and monsters.<br>Desolate: Killing a unit causes an eruption around their death location, dealing 30 + 2% of your maximum health as magic damage to all enemies nearby. This effect is 50% as effective against minions and monsters. |
| **Hubris** | 650G | +35 AD<br>+10 Ability Haste | Gain 18 Lethality.<br>Eminence: Scoring a takedown on an enemy champion generates a permanent stack and grants 12 (+3 per stack) bonus Attack Damage for 90 seconds. |
| **Hullbreaker** | 800G | +20 AD<br>+200 HP<br>+4% MS | Skipper: Every fifth basic attack against champions and monsters deals 80% AD (+5% max HP) as bonus physical damage, increased to 200% AD (+10% max HP) against turrets. Attacks from further than 35 range apply this effect at 70% strength.<br>Boarding Party: Allied minions within 100 range gain 10 armor and 20 magic resistance. |
| **Iceborn Gauntlet** | 750G | +150 HP<br>+15 Armor<br>+20% AS<br>+10 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 30 - 85 (based on level) bonus physical damage on-hit and creates a frost zone under the target for 2 seconds (1.5 second cooldown, starting after using the empowered attack). Enemies within the zone are slowed by 20%. |
| **Immortal Shieldbow** | 750G | +45 AD<br>+20% Crit Chance | Lifeline: Falling below 30% health grants a shield for 3 seconds that absorbs 330 - 605 (based on level) damage (90 second cooldown). |
| **Imperial Mandate** | 550G | +100 HP<br>+1 HP Regen<br>+25 AP<br>+15 Ability Haste | Command: Immobilizing an enemy champion marks them as Vulnerable for 3 seconds, increasing their damage taken by 9%. Subsequent applications refresh this buff. |
| **Infinity Edge** | 750G | +50 AD<br>+20% Crit Chance | Excoriate: Gain 30% critical strike damage. |
| **Jak'Sho, The Protean** | 700G | +150 HP<br>+25 Armor<br>+40 MR | Resilience: Taking damage from an enemy champion grants 6% armor and 6% magic resistance for 4 seconds (max 4 stacks). |
| **Kraken Slayer** | 700G | +30 AD<br>+30% AS<br>+4% MS | Bring It Down: Every third basic attack deals 150 bonus physical damage on-hit, increased by up to 75% based on the target's missing health (maximum bonus at 25% target health). |
| **Liandry's Torment** | 700G | +200 HP<br>+40 AP | Suffering: Dealing Ability damage burns enemies, causing them to take 6% of their maximum health as magic damage over 3 seconds. Deals a maximum of 40 magic damage per tick against minions and monsters. |
| **Lich Bane** | 750G | +50 AP<br>+20% AS<br>+10 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 105 + 30% of your Ability Power as bonus magic damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Locket of the Iron Solari** | 550G | +100 HP<br>+20 Armor<br>+30 MR<br>+10 Ability Haste | Devotion: Falling below 50% health grants you and all nearby allied champions a shield that absorbs damage equal to 170 - 225 (based on level) health over 2.5 seconds (90 second cooldown).<br>Legion: Grant 6 armor, 12 magic resistance, and 3 health regeneration to all allies within 100 range. Minions gain 150% of this value. |
| **Lord Dominik's Regards** | 750G | +25 AD<br>+20% Crit Chance<br>+25% Armor Pen | Giant Slayer: Deal 3% bonus damage for every 1000 maximum health the target has, up to 15%. |
| **Malignance** | 650G | +60 AP<br>+12 Ability Haste | Scorn: Gain 12 Ultimate Ability Haste. |
| **Mirage Blade** | 750G | +40% AS<br>+10% MS | Illusion: Gain 30 Adaptive Force. Each Adaptive Force grants 0.6 Attack Damage or 1 Ability Power, depending on which is higher.<br>Blur: On kill, gain 20% movement speed for 2 seconds. |
| **Moonstone Renewer** | 550G | +150 HP<br>+2 HP Regen<br>+20 AP<br>+15 Ability Haste | Starlit Grace: Healing an allied champion (excluding yourself) chains the effect to another allied champion within 100 range of them, healing 35% of the original amount. |
| **Morellonomicon** | 650G | +100 HP<br>+60 AP<br>+10 Ability Haste | Grievous Wounds: Dealing magic damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Mortal Reminder** | 700G | +25 AD<br>+20% Crit Chance<br>+20% Armor Pen | Grievous Wounds: Dealing physical damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Nashor's Tooth** | 750G | +60 AP<br>+25% AS | Icathian Bite: Basic attacks deal bonus magic damage on-hit equal to 35 + 3% Ability Power. |
| **Night Harvester** | 700G | +150 HP<br>+50 AP<br>+10 Ability Haste | Soulrend: Landing an Ability on an enemy champion deals bonus magic damage equal to 100 + 20% Ability Power and grants 40% movement speed for 2 seconds (45 second cooldown per target). |
| **Opportunity** | 650G | +45 AD | Gain 18 Lethality.<br>Preparation: After being out of combat with enemy champions for 7 seconds, gain 7 Lethality. This bonus remains for 3.5 seconds after dealing damage to an enemy champion. |
| **Overlord's Bloodmail** | 700G | +15 AD<br>+200 HP | Tyranny: Gain bonus Attack Damage equal to 2.5% of your maximum health. |
| **Philosopher's Stone** | 750G | +250 HP<br>+15 Armor<br>+25 MR | Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies within 30 range. This effect deals 150% more damage to minions and monsters.<br>Cinderhulk: Gain 1% maximum health for each champion takedown and monster killed, up to 15%. |
| **Protector's Vow** | 650G | +200 HP<br>+25 Armor<br>+5 Ability Haste | Awe: Gain maximum health equal to 50 + 80% of your armor. |
| **Protoplasm Harness** | 600G | +200 HP<br>+10 Ability Haste<br>+5% MS | Fortification: Falling below 40% health grants 300 + 25% of your maximum health as bonus health for 6 seconds and heals you for half that amount over 1.5 seconds (30 second cooldown). |
| **Rabadon's Deathcap** | 750G | +80 AP | Opus: Increase your total Ability Power by 20%. |
| **Randuin's Omen** | 750G | +150 HP<br>+35 Armor | Resilience: You heal for 30% of the damage taken from critical strikes. |
| **Ravenous Hydra** | 700G | +30 AD<br>+10% Omnivamp<br>+10 Ability Haste | Cleave: Basic attacks deal 30% of your Attack Damage as physical damage on-hit to nearby enemies. Attacks from further than 35 range apply this effect at 50% strength. |
| **Riftmaker** | 650G | +200 HP<br>+30 AP | Corruption: Landing an Ability on an enemy champion grants 2% Omnivamp for 3 seconds (max 3 stacks).<br>Infusion: Gain bonus Ability Power equal to 2% of your maximum health. |
| **Rite of Ruin** | 700G | +55 AP<br>+10 Ability Haste<br>+20% Crit Chance | Wrath and Ruin: Landing an Ability on an enemy champion grants 5% critical strike chance for 5 seconds (max 5 stacks).<br>Salvage the Wreckage: Landing an Ability on an enemy champion has a chance, equal to your critical strike chance, to grant you a shield for 3 seconds that absorbs 35 - 90 (based on level) damage. |
| **Rod of Ages** | 700G | +150 HP<br>+40 AP<br>+5 Ability Haste | Timeless: This item gains 10 health, 2 Ability Power and 1 Ability Haste every 30 seconds, up to 10 times.<br>Eternity: Heal for 15 - 40 (based on level) health per Ability cast. |
| **Runaan's Hurricane** | 700G | +40% AS<br>+20% Crit Chance<br>+5% MS | Wind's Fury: Basic attacks fire bolts at up to 2 additional enemies within 70 range, prioritizing champions, each dealing 50% of your Attack Damage as bonus physical damage. Bolts can critically strike. |
| **Rylai's Crystal Scepter** | 700G | +150 HP<br>+65 AP | Rimefrost: Dealing Ability damage to enemies applies a 15% slow for 2 seconds. |
| **Serpent's Fang** | 600G | +45 AD | Gain 15 Lethality.<br>Shield Reaver: Landing a basic attack or an Ability on an enemy champion with a shield deals 50 + 10% of your Attack Damage as bonus physical damage. |
| **Serylda's Grudge** | 700G | +25 AD<br>+10 Ability Haste<br>+25% Armor Pen | Bitter Cold: Dealing Ability damage to an enemy at or below 50% maximum health applies a 30% slow for 1.5 seconds. |
| **Shadowflame** | 700G | +60 AP<br>+15% Magic Pen | Cinderbloom: Your magic and true damage is 20% stronger against enemies below 40% maximum health. |
| **Solstice Sleigh** | 550G | +200 HP<br>+4 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Going Sledding: Immobilizing an enemy champion grants you and the most wounded allied champion within 100 range 20% bonus movement speed and 50 - 215 (based on level) bonus health for 2 seconds. |
| **Spear of Shojin** | 700G | +200 HP<br>+20 AD<br>+10 Ability Haste | Focused Will: Landing an Ability on an enemy champion grants 3% Attack Damage for 5 seconds (max 4 stacks). |
| **Spirit Visage** | 700G | +200 HP<br>+50 MR | Vitality: Increase all healing received by 20%. |
| **Staff of Flowing Water** | 550G | +100 HP<br>+1 HP Regen<br>+30 AP<br>+10 Ability Haste | Rapids: Healing, shielding or buffing an allied champion (excluding yourself) grants you and the target 25 Ability Power and 10 Ability Haste for 3 seconds. |
| **Statikk Shiv** | 700G | +15 AD<br>+15 AP<br>+30% AS<br>+4% MS | Energized: Moving and basic attacking generates Energize stacks, up to 100.<br>Electrospark: When fully Energized, your next basic attack fires chain lightning that deals 60 magic damage on-hit to the target and up to 4 more enemies, each within 50 range of the last, prioritizing champions. This effect is 150% as effective against minions and monsters. |
| **Sterak's Gage** | 700G | +200 HP<br>+20 AD<br>+15% Tenacity | Lifeline: Falling below 30% health grants a shield that absorbs damage equal to 45% of your maximum health for 4 seconds (90 second cooldown). |
| **Stormrazor** | 800G | +35 AD<br>+20% AS<br>+20% Crit Chance | Energized: Moving and basic attacking generates Energize stacks, up to 100.<br>Bolt: When fully Energized, your next basic attack deals 100 bonus magic damage on-hit and grants you 35% movement speed for 1.5 seconds. |
| **Stormsurge** | 700G | +55 AP<br>+5% MS<br>+10% Magic Pen | Stormraider: Dealing damage to an enemy champion equal to 25% of their maximum health within 2.5 seconds inflicts them with Squall (30 second cooldown per target).<br>Squall: After 2 seconds, strike the target with lightning, dealing 125 + 10% of your Ability Power as magic damage. |
| **Sundered Sky** | 700G | +200 HP<br>+20 AD<br>+10 Ability Haste | Lightshield Strike: Your next basic attack against an enemy champion critically strikes for 60% bonus damage and heals you for 60 + 6% of your missing health (20 second cooldown per target). |
| **Sword of Blossoming Dawn** | 500G | +100 HP<br>+20 AP<br>+20% AS<br>+10 Ability Haste | Peppermint: Basic attacks heal the most wounded and nearest ally champion for 15 - 60 (based on level) (+7% AD) (+7% AP). |
| **Terminus** | 700G | +15 AD<br>+35% AS<br>+20% Crit Chance | Juxtaposition: Basic attacks grant either 4% armor penetration or 4% magic resistance penetration for 4 seconds, alternating (max 4 stacks each). |
| **Titanic Hydra** | 750G | +200 HP<br>+25 AD | Cleave: Basic attacks deal 1% of your maximum health as bonus physical damage on-hit to the target and 3% as physical damage to enemies behind the target. Attacks from further than 35 range apply this effect at 50% strength. |
| **Trinity Force** | 750G | +100 HP<br>+20 AD<br>+20% AS<br>+10 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 33 + 33% of your Attack Damage as bonus physical damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Unending Despair** | 750G | +250 HP<br>+15 Armor | Anguish: Landing an Ability on an enemy champion heals you for 35 + 1% of your maximum health. |
| **Void Staff** | 750G | +50 AP<br>+25% Magic Pen | — |
| **Voltaic Cyclosword** | 650G | +35 AD<br>+10 Ability Haste | Gain 12 Lethality.<br>Energized: Moving and basic attacking generates Energize stacks, up to 100.<br>Firmament: When fully Energized, your next basic attack grants you 6 Lethality for 4 seconds and deals bonus physical damage on-hit equal to 6% of the target's current health. Deals a maximum of 200 physical damage against minions and monsters. |
| **Warmog's Armor** | 750G | +300 HP<br>+3 HP Regen | Warmog's Heart: Regenerate 3% of your maximum health every second and gain 4% movement speed if you have not taken damage in the last 6 seconds. |
| **Wit's End** | 700G | +40% AS<br>+40 MR<br>+20% Tenacity | Fray: Basic attacks deal 45 bonus magic damage on-hit. |
| **Yun Tal Wildarrows** | 750G | +35 AD<br>+20% AS | Practice Makes Lethal: Basic attacks grant 1% critical strike chance permanently, up to 25%.<br>Flurry: Every 15 seconds, your next basic attack grants 30% attack speed for 6 seconds. |
| **Zaz'Zak's Realmspike** | 550G | +200 HP<br>+4 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Void Explosion: Dealing ability damage to an enemy champion creates an explosion at their location after a 0.5 second delay, dealing 10 (+15% AP) (+3% of the target's max HP) magic damage to enemies hit (10 second cooldown). |
| **Zeke's Convergence** | 550G | +100 HP<br>+20 Armor<br>+30 MR<br>+10 Ability Haste | Cryocombustion: Gain 15 Ultimate Ability Haste.<br>Frostfire Tempest: Upon casting your ultimate ability, summon a storm of flame and ice around you for 4 seconds. The storm deals 60 magic damage per second to nearby enemies and applies a 40% slow. |
| **Zhonya's Hourglass** | 750G | +50 AP<br>+35 Armor | Time Stop: Falling below 20% health puts you in stasis for 2.5 seconds. While in stasis, you are untargetable, invulnerable, and unable to act (120 second cooldown). |


#### Tier 5  
| Item | Cost | Stats | Passive |
| --- | --- | --- | --- |
| **Radiant Abyssal Mask** | 950G | +250 HP<br>+75 MR<br>+15 Ability Haste | Unmake: Reduce the magic resistance of enemy champions within 50 range by 20%. |
| **Radiant Ardent Censer** | 850G | +200 HP<br>+2 HP Regen<br>+45 AP<br>+5 Ability Haste<br>+5% MS | Sanctify: Healing, shielding or buffing an allied champion (excluding yourself) grants them 20% attack speed and bonus physical damage on-hit equal to 2% of the target's maximum health for 6 seconds. |
| **Radiant Atma's Reckoning** | 1050G | +450 HP<br>+25% Crit Chance | Big Hands: Gain 5% critical strike chance for every 1000 maximum health, up to 25%. |
| **Radiant Axiom Arc** | 950G | +55 AD<br>+15 Ability Haste | Gain 18 Lethality.<br>Flux: Gain 10 (+0.2 per 1 Lethality) Ultimate Ability Haste. |
| **Radiant Bandlepipes** | 750G | +200 HP<br>+30 Armor<br>+50 MR<br>+20 Ability Haste | Fanfare: Dealing damage to an enemy champion empowers you with Fanfare for 4 seconds, granting you 12% bonus movement speed. While empowered, you and allied champions within 100 range also gain 20% attack speed. |
| **Radiant Banshee's Veil** | 950G | +100 AP<br>+60 MR | Annul: Grants a spell shield that blocks the next enemy Ability (40 second cooldown). |
| **Radiant Bastionbreaker** | 1000G | +55 AD<br>+20 Ability Haste | Gain 22 Lethality.<br>Sabotage: Scoring a takedown on an enemy champion grants Sabotage for 90 seconds, empowering your next basic attack against a turret to deal 200 + 20% of your Attack Damage as bonus true damage. |
| **Radiant Black Cleaver** | 1100G | +35 AD<br>+250 HP<br>+10 Ability Haste | Carve: Dealing physical damage to enemy champions reduces their armor by 6% for 6 seconds (max 5 stacks). |
| **Radiant Blackfire Torch** | 950G | +90 AP<br>+25 Ability Haste | Maleficent: Landing an Ability on an enemy champion grants 10 Ability Power for 4 seconds (max 4 stacks). |
| **Radiant Blade of the Ruined King** | 1050G | +30 AD<br>+50% AS<br>+10% Omnivamp | Mist's Edge: Basic attacks deal bonus physical damage on-hit equal to 8% of the target's current health. Deals a maximum of 50 physical damage against minions and monsters. |
| **Radiant Bloodletter's Curse** | 1100G | +250 HP<br>+90 AP<br>+10 Ability Haste | Decay: Dealing magic damage to enemy champions reduces their magic resistance by 6% for 6 seconds (max 5 stacks). |
| **Radiant Bloodsong** | 750G | +300 HP<br>+5 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 70 - 125 (based on level) bonus magic damage on-hit (1.5 second cooldown, starting after using the empowered attack). If the target is a champion, increase their damage taken by 7% for 4 seconds. |
| **Radiant Celestial Opposition** | 750G | +300 HP<br>+5 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Blessing of the Mountain: Become Blessed upon taking damage from an enemy champion, reducing your damage taken by 15% for 2 seconds. After Blessed ends, unleash a shockwave around you that slows enemies nearby by 50% for 1.5 seconds (18 second cooldown, beginning after the explosion). |
| **Radiant Chempunk Chainsword** | 950G | +30 AD<br>+300 HP<br>+15 Ability Haste | Grievous Wounds: Dealing physical damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Radiant Chemtech Putrifier** | 750G | +250 HP<br>+3 HP Regen<br>+25 AP<br>+15 Ability Haste | Grievous Wounds: Dealing damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Radiant Cloak of Starry Night** | 1050G | +250 HP<br>+100 MR<br>+25% Total MR | Limitless as the Stars: Increase your total magic resistance by 25%. Additionally, gain 5% (+1% per 25 magic resistance) skill damage reduction, up to a maximum of 25%. |
| **Radiant Collector** | 1050G | +65 AD<br>+25% Crit Chance | Gain 10 Lethality.<br>Death: Dealing damage to enemy champions below 6% maximum health executes them.<br>Taxes: Killing a champion grants you an additional 25 gold. |
| **Radiant Cosmic Drive** | 1000G | +250 HP<br>+75 AP<br>+15 Ability Haste<br>+4% MS | Spelldance: Dealing magic damage or true damage to an enemy champion grants you 8% movement speed for 4 seconds. |
| **Radiant Dead Man's Plate** | 1050G | +350 HP<br>+35 Armor<br>+4% MS | Shipwrecker: While moving, generate 7 stacks of Momentum every second, up to 100. Each stack grants 0.25% bonus movement speed. Basic attacks consume all remaining Momentum to deal 0 - 200 bonus physical damage on-hit, scaling with stacks consumed. |
| **Radiant Deathblade** | 1000G | +80 AD | Apex: Increase your total Attack Damage by 25%. |
| **Radiant Death's Dance** | 1050G | +45 AD<br>+45 Armor<br>+15 Ability Haste | Ignore Pain: 25% of the damage you take is dealt over time as true damage (up to 2.5% of your maximum health per second).<br>Defy: Scoring a takedown on an enemy champion cleanses the remaining stored damage and heals you for 75 + 25% of your missing health. |
| **Radiant Diamond Tipped Spear** | 1150G | +60% AS<br>+10 Ability Haste | Pierce: Gain 50 Adaptive Force. Each Adaptive Force grants 0.6 Attack Damage or 1 Ability Power, depending on which is higher.<br>Sweet Spot: Deal up to 25% bonus damage to enemy champions based on distance (maximum effect at 100 range). |
| **Radiant Dream Maker** | 750G | +300 HP<br>+5 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Dream Maker: Healing, shielding, or buffing an allied champion grants them a Dream Bubble for 5 seconds. While the Dream Bubble is active, it grants 8% skill damage reduction and 20 Adaptive Force (8 second cooldown, starting after the Dream Bubble ends). |
| **Radiant Dusk and Dawn** | 1000G | +150 HP<br>+75 AP<br>+25% AS<br>+10 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 85 + 15% of your Ability Power as bonus magic damage on-hit and heal you for 10% of your Ability Power and 2.5% of your maximum health (1.5 second cooldown, starting after using the empowered attack). |
| **Radiant Echoes of Helia** | 750G | +250 HP<br>+3 HP Regen<br>+35 AP<br>+20 Ability Haste | Soul Siphon: Store 30% of the damage you deal or take as Soul Charges, up to 130 - 350 (based on level). Healing, shielding or buffing an allied champion (excluding yourself) consumes all Soul Charges and heals them equal to the consumed amount. |
| **Radiant Eclipse** | 1000G | +65 AD<br>+15 Ability Haste | Ever Rising Moon: Landing a basic attack or an Ability on an enemy champion marks them for 2 seconds, up to once per cast instance. Hitting a marked champion consumes the mark to deal bonus physical damage equal to 8% of their maximum health and grant you a shield that absorbs 120 + 20% of your Attack Damage for 2 seconds (6 second cooldown per target). |
| **Radiant Edge of Night** | 1000G | +60 AD<br>+150 HP | Gain 15 Lethality.<br>Annul: Grants a spell shield that blocks the next enemy Ability (40 second cooldown). |
| **Radiant Endless Hunger** | 1000G | +60 AD<br>+10% Omnivamp<br>+25% Tenacity | Famine: Gain 5 (+5% AD) Ability Haste.<br>Feast: Scoring a takedown on an enemy champion grants 15% Omnivamp for 5 seconds. |
| **Radiant Essence Reaver** | 1000G | +50 AD<br>+30% AS<br>+15 Ability Haste<br>+25% Crit Chance | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 125% of your Attack Damage (+100% Crit Chance) as bonus physical damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Radiant Experimental Hexplate** | 950G | +200 HP<br>+50% AS<br>+5% MS | Hexcharged: Gain 25 Ultimate Ability Haste.<br>Overdrive: Upon casting your ultimate ability, enter Overdrive, gaining 32% attack speed and 16% movement speed for 8 seconds (32 second cooldown, starting on cast). |
| **Radiant Feral Flare** | 1000G | +30 AD<br>+40% AS<br>+20 Armor | Maim: Gain a Feral stack for each champion takedown scored and monster killed, up to 50. Basic attacks deal 25 (+1 per Feral stack) bonus magic damage and restore 10 health. This effect is 150% as effective against minions and monsters. |
| **Radiant Fiendhunter Bolts** | 1000G | +55% AS<br>+25% Crit Chance<br>+4% MS | Night Vigil: Gain 25 Ultimate Ability Haste.<br>Opening Barrage: Upon casting your ultimate ability, gain 50% attack speed for 8 seconds. Your next 3 basic attacks within this window critically strike for 60% bonus damage. If the attack would have already critically struck, it instead deals bonus true damage equal to 15% of its damage dealt (45 second cooldown). |
| **Radiant Frozen Heart** | 950G | +55 Armor<br>+15 Ability Haste<br>+15% Skill DMG Reduction | Winter's Caress: Reduce the attack speed of enemy champions within 100 range by 30%. |
| **Radiant Frozen Mallet** | 1000G | +300 HP<br>+30 AD | Icy: Basic attacks deal bonus physical damage equal to 20 + 3% of your maximum health and apply a 15% slow for 2 seconds. |
| **Radiant Grez's Spectral Lantern** | 1000G | +200 HP<br>+60 AP<br>+10 Ability Haste | Spirit Drain: Gain 1 Ability Power for each champion takedown and monster killed, up to 40.<br>Butcher: Against monsters, deal 30% bonus magic damage and restore health equal to 6% of your damage dealt. |
| **Radiant Guardian Angel** | 1100G | +50 AD<br>+45 Armor | Rebirth: Upon taking lethal damage, instead resurrect for 4 seconds, healing for 60% of your maximum health. While resurrecting, you are untargetable, invulnerable, and unable to act (300 second cooldown). |
| **Radiant Guinsoo's Rageblade** | 950G | +25 AD<br>+25 AP<br>+50% AS | Wrath: Basic attacks deal 30 bonus magic damage on-hit.<br>Seething Strike: Basic attacks grant 8% attack speed for 4 seconds (max 4 stacks). |
| **Radiant Hamstringer** | 1100G | +40 AD<br>+45% AS<br>+25% Crit Chance | Scour: Your critical strikes bleed the target, dealing 70 - 180 (based on level) (+100% Crit Chance) physical damage over 2 seconds and applying a 7% slow. |
| **Radiant Heartsteel** | 1050G | +400 HP | Ironheart: If an enemy champion stays within 50 range of you for 1.5 seconds, your next basic attack against them deals bonus physical damage on-hit equal to 70 + 6% of your maximum health, granting 10% of that damage as permanent bonus health (30 second cooldown per target).<br>Goliath: Gain 3% increased size for every 1000 maximum health, up to 30%. |
| **Radiant Hextech Gunblade** | 1050G | +45 AD<br>+75 AP<br>+15% Omnivamp | — |
| **Radiant Hollow Radiance** | 1050G | +350 HP<br>+70 MR<br>+6% Skill DMG Reduction | Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies within 30 range. This effect deals 50% more damage to minions and monsters.<br>Desolate: Killing a unit causes an eruption around their death location, dealing 60 + 4% of your maximum health as magic damage to all enemies nearby. This effect is 50% as effective against minions and monsters. |
| **Radiant Hubris** | 1000G | +60 AD<br>+15 Ability Haste | Gain 18 Lethality.<br>Eminence: Scoring a takedown on an enemy champion generates a permanent stack and grants 12 (+3 per stack) bonus Attack Damage for 90 seconds. |
| **Radiant Hullbreaker** | 1000G | +35 AD<br>+300 HP<br>+4% MS | Skipper: Every fifth basic attack against champions and monsters deals 80% AD (+5% max HP) as bonus physical damage, increased to 200% AD (+10% max HP) against turrets. Attacks from further than 35 range apply this effect at 70% strength.<br>Boarding Party: Allied minions within 100 range gain 10 armor and 20 magic resistance. |
| **Radiant Iceborn Gauntlet** | 1000G | +250 HP<br>+25 Armor<br>+25% AS<br>+15 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 30 - 85 (based on level) bonus physical damage on-hit and creates a frost zone under the target for 2 seconds (1.5 second cooldown, starting after using the empowered attack). Enemies within the zone are slowed by 20%. |
| **Radiant Immortal Shieldbow** | 1050G | +65 AD<br>+25% Crit Chance | Lifeline: Falling below 30% health grants a shield for 3 seconds that absorbs 330 - 605 (based on level) damage (90 second cooldown). |
| **Radiant Imperial Mandate** | 750G | +150 HP<br>+2 HP Regen<br>+40 AP<br>+20 Ability Haste | Command: Immobilizing an enemy champion marks them as Vulnerable for 3 seconds, increasing their damage taken by 9%. Subsequent applications refresh this buff. |
| **Radiant Infinity Edge** | 1150G | +75 AD<br>+25% Crit Chance | Excoriate: Gain 30% critical strike damage. |
| **Radiant Jak'Sho, The Protean** | 1000G | +300 HP<br>+35 Armor<br>+60 MR | Resilience: Taking damage from an enemy champion grants 10% armor and 10% magic resistance for 4 seconds (max 4 stacks). |
| **Radiant Kraken Slayer** | 1000G | +45 AD<br>+45% AS<br>+4% MS | Bring It Down: Every third basic attack deals 150 bonus physical damage on-hit, increased by up to 75% based on the target's missing health (maximum bonus at 25% target health). |
| **Radiant Liandry's Torment** | 1000G | +300 HP<br>+75 AP | Suffering: Dealing Ability damage burns enemies, causing them to take 6% of their maximum health as magic damage over 3 seconds. Deals a maximum of 40 magic damage per tick against minions and monsters. |
| **Radiant Lich Bane** | 1050G | +90 AP<br>+25% AS<br>+15 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 105 + 45% of your Ability Power as bonus magic damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Radiant Locket of the Iron Solari** | 850G | +150 HP<br>+40 Armor<br>+50 MR<br>+15 Ability Haste | Devotion: Falling below 50% health grants you and all nearby allied champions a shield that absorbs damage equal to 295 - 350 (based on level) health over 2.5 seconds (90 second cooldown).<br>Legion: Grant 6 armor, 12 magic resistance, and 3 health regeneration to all allies within 100 range. Minions gain 150% of this value. |
| **Radiant Lord Dominik's Regards** | 1000G | +45 AD<br>+25% Crit Chance<br>+35% Armor Pen | Giant Slayer: Deal 3% bonus damage for every 1000 maximum health the target has, up to 15%. |
| **Radiant Malignance** | 950G | +100 AP<br>+20 Ability Haste | Scorn: Gain 20 Ultimate Ability Haste. |
| **Radiant Mirage Blade** | 1050G | +65% AS<br>+15% MS | Illusion: Gain 50 Adaptive Force. Each Adaptive Force grants 0.6 Attack Damage or 1 Ability Power, depending on which is higher.<br>Blur: On kill, gain 20% movement speed for 2 seconds. |
| **Radiant Moonstone Renewer** | 750G | +250 HP<br>+3 HP Regen<br>+30 AP<br>+20 Ability Haste | Starlit Grace: Healing an allied champion (excluding yourself) chains the effect to another allied champion within 100 range of them, healing 35% of the original amount. |
| **Radiant Morellonomicon** | 950G | +200 HP<br>+95 AP<br>+10 Ability Haste | Grievous Wounds: Dealing magic damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Radiant Mortal Reminder** | 1000G | +45 AD<br>+25% Crit Chance<br>+30% Armor Pen | Grievous Wounds: Dealing physical damage to an enemy champion reduces their healing by 40% for 2 seconds. |
| **Radiant Nashor's Tooth** | 1050G | +90 AP<br>+40% AS | Icathian Bite: Basic attacks deal bonus magic damage on-hit equal to 50 + 5% Ability Power. |
| **Radiant Night Harvester** | 1000G | +250 HP<br>+80 AP<br>+10 Ability Haste | Soulrend: Landing an Ability on an enemy champion deals bonus magic damage equal to 150 + 30% Ability Power and grants 40% movement speed for 2 seconds (45 second cooldown per target). |
| **Radiant Opportunity** | 1000G | +65 AD<br>+5% MS | Gain 18 Lethality.<br>Preparation: After being out of combat with enemy champions for 7 seconds, gain 7 Lethality. This bonus remains for 3.5 seconds after dealing damage to an enemy champion. |
| **Radiant Overlord's Bloodmail** | 1000G | +20 AD<br>+350 HP | Tyranny: Gain bonus Attack Damage equal to 2.5% of your maximum health. |
| **Radiant Philosopher's Stone** | 1050G | +350 HP<br>+30 Armor<br>+45 MR | Immolate: Deal 10 + 1% of your maximum health as magic damage to all enemies within 30 range. This effect deals 150% more damage to minions and monsters.<br>Cinderhulk: Gain 1% maximum health for each champion takedown and monster killed, up to 25%. |
| **Radiant Protector's Vow** | 900G | +300 HP<br>+40 Armor<br>+15 Ability Haste | Awe: Gain maximum health equal to 50 + 80% of your armor. |
| **Radiant Protoplasm Harness** | 850G | +350 HP<br>+10 Ability Haste<br>+5% MS | Fortification: Falling below 40% health grants 600 + 25% of your maximum health as bonus health for 6 seconds and heals you for half that amount over 1.5 seconds (30 second cooldown). |
| **Radiant Rabadon's Deathcap** | 1150G | +130 AP | Opus: Increase your total Ability Power by 35%. |
| **Radiant Randuin's Omen** | 1000G | +300 HP<br>+45 Armor | Resilience: You heal for 30% of the damage taken from critical strikes. |
| **Radiant Ravenous Hydra** | 950G | +45 AD<br>+15% Omnivamp<br>+15 Ability Haste | Cleave: Basic attacks deal 40% of your Attack Damage as physical damage on-hit to nearby enemies. Attacks from further than 35 range apply this effect at 50% strength. |
| **Radiant Riftmaker** | 950G | +300 HP<br>+60 AP | Corruption: Landing an Ability on an enemy champion grants 2% Omnivamp for 3 seconds (max 3 stacks).<br>Infusion: Gain bonus Ability Power equal to 2% of your maximum health. |
| **Radiant Rite of Ruin** | 1000G | +95 AP<br>+15 Ability Haste<br>+25% Crit Chance | Wrath and Ruin: Landing an Ability on an enemy champion grants 5% critical strike chance for 5 seconds (max 5 stacks).<br>Salvage the Wreckage: Landing an Ability on an enemy champion has a chance, equal to your critical strike chance, to grant you a shield for 3 seconds that absorbs 65 - 175 (based on level) damage. |
| **Radiant Rod of Ages** | 1000G | +250 HP<br>+65 AP<br>+10 Ability Haste | Timeless: This item gains 10 health, 2 Ability Power and 1 Ability Haste every 30 seconds, up to 10 times.<br>Eternity: Heal for 15 - 40 (based on level) health per Ability cast. |
| **Radiant Runaan's Hurricane** | 950G | +70% AS<br>+25% Crit Chance<br>+5% MS | Wind's Fury: Basic attacks fire bolts at up to 2 additional enemies within 70 range, prioritizing champions, each dealing 50% of your Attack Damage as bonus physical damage. Bolts can critically strike. |
| **Radiant Rylai's Crystal Scepter** | 950G | +200 HP<br>+100 AP | Rimefrost: Dealing Ability damage to enemies applies a 15% slow for 2 seconds. |
| **Radiant Serpent's Fang** | 900G | +70 AD | Gain 15 Lethality.<br>Shield Reaver: Landing a basic attack or an Ability on an enemy champion with a shield deals 85 + 15% of your Attack Damage as bonus physical damage. |
| **Radiant Serylda's Grudge** | 1050G | +45 AD<br>+15 Ability Haste<br>+35% Armor Pen | Bitter Cold: Dealing Ability damage to an enemy at or below 50% maximum health applies a 30% slow for 1.5 seconds. |
| **Radiant Shadowflame** | 900G | +105 AP<br>+15% Magic Pen | Cinderbloom: Your magic and true damage is 20% stronger against enemies below 40% maximum health. |
| **Radiant Solstice Sleigh** | 750G | +300 HP<br>+5 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Going Sledding: Immobilizing an enemy champion grants you and the most wounded allied champion within 100 range 20% bonus movement speed and 50 - 215 (based on level) bonus health for 2 seconds. |
| **Radiant Spear of Shojin** | 1100G | +300 HP<br>+30 AD<br>+20 Ability Haste | Focused Will: Landing an Ability on an enemy champion grants 3% Attack Damage for 5 seconds (max 4 stacks). |
| **Radiant Spirit Visage** | 950G | +300 HP<br>+75 MR | Vitality: Increase all healing received by 20%. |
| **Radiant Staff of Flowing Water** | 750G | +150 HP<br>+2 HP Regen<br>+50 AP<br>+15 Ability Haste | Rapids: Healing, shielding or buffing an allied champion (excluding yourself) grants you and the target 25 Ability Power and 10 Ability Haste for 3 seconds. |
| **Radiant Statikk Shiv** | 1000G | +25 AD<br>+25 AP<br>+50% AS<br>+4% MS | Energized: Moving and basic attacking generates Energize stacks, up to 100.<br>Electrospark: When fully Energized, your next basic attack fires chain lightning that deals 80 magic damage on-hit to the target and up to 5 more enemies, each within 50 range of the last, prioritizing champions. This effect is 150% as effective against minions and monsters. |
| **Radiant Sterak's Gage** | 1000G | +350 HP<br>+25 AD<br>+20% Tenacity | Lifeline: Falling below 30% health grants a shield that absorbs damage equal to 45% of your maximum health for 4 seconds (90 second cooldown). |
| **Radiant Stormrazor** | 1100G | +50 AD<br>+40% AS<br>+25% Crit Chance | Energized: Moving and basic attacking generates Energize stacks, up to 100.<br>Bolt: When fully Energized, your next basic attack deals 100 bonus magic damage on-hit and grants you 35% movement speed for 1.5 seconds. |
| **Radiant Stormsurge** | 1000G | +100 AP<br>+5% MS<br>+15% Magic Pen | Stormraider: Dealing damage to an enemy champion equal to 25% of their maximum health within 2.5 seconds inflicts them with Squall (30 second cooldown per target).<br>Squall: After 2 seconds, strike the target with lightning, dealing 125 + 15% of your Ability Power as magic damage. |
| **Radiant Sundered Sky** | 1000G | +300 HP<br>+35 AD<br>+20 Ability Haste | Lightshield Strike: Your next basic attack against an enemy champion critically strikes for 60% bonus damage and heals you for 60 + 10% of your missing health (20 second cooldown per target). |
| **Radiant Sword of Blossoming Dawn** | 850G | +150 HP<br>+35 AP<br>+35% AS<br>+15 Ability Haste | Peppermint: Basic attacks heal the most wounded and nearest ally champion for 15 - 60 (based on level) (+7% AD) (+7% AP). |
| **Radiant Terminus** | 1000G | +25 AD<br>+60% AS<br>+25% Crit Chance | Juxtaposition: Basic attacks grant either 4% armor penetration or 4% magic resistance penetration for 4 seconds, alternating (max 4 stacks each). |
| **Radiant Titanic Hydra** | 1000G | +300 HP<br>+40 AD | Cleave: Basic attacks deal 1.5% of your maximum health as bonus physical damage on-hit to the target and 4% as physical damage to enemies behind the target. Attacks from further than 35 range apply this effect at 50% strength. |
| **Radiant Trinity Force** | 1333G | +333 HP<br>+33 AD<br>+25% AS<br>+15 Ability Haste | Spellblade: Using an Ability causes your next basic attack within 10 seconds to deal 33 + 33% of your Attack Damage as bonus physical damage on-hit (1.5 second cooldown, starting after using the empowered attack). |
| **Radiant Unending Despair** | 1050G | +350 HP<br>+25 Armor | Anguish: Landing an Ability on an enemy champion heals you for 50 + 2.5% of your maximum health. |
| **Radiant Void Staff** | 1100G | +80 AP<br>+40% Magic Pen | — |
| **Radiant Voltaic Cyclosword** | 1000G | +60 AD<br>+15 Ability Haste | Gain 12 Lethality.<br>Energized: Moving and basic attacking generates Energize stacks, up to 100.<br>Firmament: When fully Energized, your next basic attack grants you 10 Lethality for 4 seconds and deals bonus physical damage on-hit equal to 10% of the target's current health. Deals a maximum of 200 physical damage against minions and monsters. |
| **Radiant Warmog's Armor** | 1050G | +500 HP<br>+5 HP Regen | Warmog's Heart: Regenerate 3% of your maximum health every second and gain 4% movement speed if you have not taken damage in the last 6 seconds. |
| **Radiant Wit's End** | 1000G | +65% AS<br>+65 MR<br>+30% Tenacity | Fray: Basic attacks deal 45 bonus magic damage on-hit. |
| **Radiant Yun Tal Wildarrows** | 1100G | +40 AD<br>+50% AS | Practice Makes Lethal: Basic attacks grant 1% critical strike chance permanently, up to 25%.<br>Flurry: Every 15 seconds, your next basic attack grants 30% attack speed for 6 seconds. |
| **Radiant Zaz'Zak's Realmspike** | 750G | +300 HP<br>+5 HP Regen | Shared Riches: Gain 4 gold every 5 seconds.<br>Void Explosion: Dealing ability damage to an enemy champion creates an explosion at their location after a 0.5 second delay, dealing 10 (+15% AP) (+3% of the target's max HP) magic damage to enemies hit (10 second cooldown). |
| **Radiant Zeke's Convergence** | 750G | +150 HP<br>+30 Armor<br>+40 MR<br>+15 Ability Haste | Cryocombustion: Gain 15 Ultimate Ability Haste.<br>Frostfire Tempest: Upon casting your ultimate ability, summon a storm of flame and ice around you for 4 seconds. The storm deals 60 magic damage per second to nearby enemies and applies a 40% slow. |
| **Radiant Zhonya's Hourglass** | 1050G | +80 AP<br>+50 Armor | Time Stop: Falling below 20% health puts you in stasis for 2.5 seconds. While in stasis, you are untargetable, invulnerable, and unable to act (120 second cooldown). |

### Base Item Reskins

| Original Name | New Name |
| --- | --- |
| Iron Sword | Long Sword |
| Soldier's Longsword | Pickaxe |
| Ruinous Blade | Vampiric Scepter |
| Conqueror's Greatsword | Bloodthirster |
| Warlord's Final Judgement | Radiant Bloodthirster |
| Dagger | Dagger |
| Wind Dagger | Recurve Bow |
| Twin Stormblade | Zeal |
| Thunderclaw | Phantom Dancer |
| Storm Sovereign | Radiant Phantom Dancer |
| Steel Armor | Cloth Armor |
| Gatekeeper's Armor | Chain Vest |
| Black Knight's Heavy Plate | Bramble Vest |
| Eternal Iron Plate | Thornmail |
| Impregnable Fortress | Radiant Thornmail |
| Mystic Cloak | Null-Magic Mantle |
| Night Hood | Negatron Cloak |
| Dusk Raven | Spectre's Cowl |
| Soul's Edge | Dragon's Claw |
| Veil of Annihilation | Radiant Dragon's Claw |
| Arcane Crystal | Amplifying Tome |
| Spirit Crystal | Blasting Wand |
| Staff of Rapture | Lost Chapter |
| Angel's Fang | Luden's Tempest |
| Prophet of the Abyss | Radiant Luden's Tempest |
| Vital Orb | Ruby Crystal |
| Hardened Heart | Kindlegem |
| Ring of Reincarnation | Giant's Belt |
| Hourglass of Eternity | Sunfire Cape |
| Giant's Horn Shard | Radiant Sunfire Cape |
