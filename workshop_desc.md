Adds 6 item slots & 237 new items (138 base + 99 Radiant) to Teamfight Manager 2.  
Also re-skins the 30 existing items and adds custom icons for Armor Penetration, Magic Penetration, Ability Haste, Tenacity, Omnivamp, and Skill Damage Reduction.  
Champions start each match with 250 gold instead of 500.  

[b]Supports custom item values, custom item builds, and Smart Builds for the AI. See below![/b]  

[h1] Important [/h1]
This mod currently supports English, Vietnamese, Portuguese (BR), Russian, Chinese (Simplified), and Korean locales. You can use it with other languages, but the names and descriptions of the items will be broken.  

If you would like to provide translations, feel free to shoot me a message on Discord @shirograhm.  

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.  

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Versioning [/h1]
Currently updated for game version 0.6.3. Older game versions need an older build of the mod:  
Mod v0.11.11+ - 0.6.3  
Mod v0.11.3-0.11.10 - 0.6.2  
Mod v0.10.1-0.11.2 - 0.6.1  
Mod v0.9.11-0.10.0 - 0.6.0  

Manual releases: https://github.com/shirograhm/riot-items-tfm2/releases

[h1] Instructions [/h1]
If you are only seeing Bloodthirster/Luden's/Sunfire (vanilla items, no modded), that means the save you are playing is not loading the mod order. To fix this, try the following:

1. Save your current game and go back to the main menu.
2. Click Load -> Load on the save you just made. It will probably say "No Info" under mods.
2. Once launched, progress once and save again. Then go back to the main menu.
3. Click Load -> Load again, this time the mods column should read "Match". <-- (That means it's loading correctly)
4. Play as you would normally!

[h1] Known Issues [/h1]
- Some AI champions prefer the wrong stats when picking their own items. Keep [b]Enforce Smart Builds[/b] on to prevent this.  
- The build editor only shows item and champion names in English.  
- Largely untested in multiplayer. It should work, but custom item builds only follow the host's choices.  

[h1] Custom Item Values [/h1]
This mod works directly out of the box! However, if any of the items feel too strong/weak, this mod supports full customization on all item values. To do so:  

1. Make a copy of the [b]config-default.json[/b] that ships with this mod, and name it [b]config.json[/b]. [i]Make sure to name it exactly or else this will not work.[/i]  
2. Edit the new [b]config.json[/b] with the custom values that you want.  
3. Re-run the game and open your save. No need to disable/re-enable the mod if you already had it running in the save!  
4. If you changed any of the base game's own items (Bloodthirster, Thornmail and the like), start the game twice: the first start writes the new values, the second one loads them.  

Your config.json is your item information save. If you lose it, you can re-copy the default values from config-default.json. Otherwise, the game will run with the default hardcoded values.  

Both files (config-default.json & config.json) should be located in the mod's workshop folder in your SteamLibrary: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]

[h1] Item Patches [/h1]
Whenever the game patches its champions, this mod patches its items too: items that won and were built too often are nerfed, and items that did neither are buffed. The changes arrive in your inbox as [b]Item Patch Notes[/b]. Boots, jungle items and support-only items are judged on win rate alone, since Smart Builds decides who builds them. A patch may change an item's base version, its Radiant version, or both, and not always in the same stats. A Radiant item's passive is never left weaker than its base item's.  

Don't want to wait for the next champion patch? Open [b]Options > Competition[/b] from the pause menu and click [b]Queue[/b] next to [b]Run Manual Item Patch[/b]. A manual item patch is released with the next news that reaches your inbox, as a hotfix of the current version. The button stays greyed out until it has been released.  

To tune the patch system, make a copy of the [b]config-autopatch-default.json[/b] that ships with this mod (same folder as config-default.json), and name it [b]config-autopatch.json[/b]. In the copy you can change:  
- [b]enabled[/b]: set to false to turn item patches off.  
- [b]automatic[/b]: set to false to stop item patches from arriving with the champion patches. Run Manual Item Patch still works, and patches you already have stay in effect.  
- [b]step_size[/b]: how much each stat changes at a time (for example 50 health, 5 attack damage). Set a stat to 0 to prevent item patches from changing it.  
- [b]limits[/b]: how many games an item needs before it can be patched, how many items one patch nerfs and buffs, and how far an item's numbers may move from the original in total (65% to 140% by default).  

Re-run the game after editing it. Without a config-autopatch.json, and for anything yours leaves out, the mod uses config-autopatch-default.json.  

[h1] Custom Item Builds [/h1]
Pick any item for any champion, in-game:
1. After draft, on the Item Strategy Screen, click [b]Builds[/b] at the top.  
2. Press [b]+ Add Champion[/b], pick a champion (modded champions included), and set its item slots. Any slot left on [b]Let Player Decide (-)[/b] is filled by the AI.  
3. Start the match!  

Use the [b]filter by champion[/b] box to find champions in a long list (separate several with commas). After a draft, [b]Blue Team[/b] and [b]Red Team[/b] under the Matchup card fill the box with that side's champions. Builds save automatically to [b]item-builds.json[/b] and carry across sessions. [b]Save Item Builds[/b] saves manually.  

During a match, you can see both sides' planned builds using the [b]Check Tactics[/b] tab. Pinned items will render with a teal border.  

[h1] Smart Builds [/h1]
[b]Enforce Smart Builds[/b] cleans up the AI's picks. The following items are swapped for another item of the same category:
- duplicates, a second Grievous Wounds item, or crit past 100% (passive crit counts as fully stacked)
- a second Spellblade item (Trinity Force, Dusk and Dawn, Lich Bane, Essence Reaver, Iceborn Gauntlet, Bloodsong). Sheen doesn't count.
- 1 boots item per player in the second slot (unless pinned elsewhere in the build)
- support items (except Protoplasm Harness) outside the support role. tank supports and heal/shield/buff supports (the ones that build Dream Maker) are restricted to Tank/Support items, and the latter prefer Ardent Censer, Echoes of Helia and Staff of Flowing Water
- heal, shield and buff items (Ardent Censer, Echoes of Helia, Moonstone Renewer, Staff of Flowing Water) on supports that don't heal, shield or buff allies, and on mages
- jungle items (Feral Flare, Grez's Spectral Lantern, Philosopher's Stone) outside the jungle role
- melee items (Ravenous Hydra, Titanic Hydra, Hullbreaker, Heartsteel, Abyssal Mask, Sunfire Cape, Hollow Radiance, Philosopher's Stone) on ranged champions, and ranged items (Runaan's Hurricane, Diamond Tipped Spear) on melee champions. Melee means an attack range of 35 or less.
- items the champion doesn't scale with: attack items on an AP champion, AP items on an AD champion, and Hextech Gunblade (the one hybrid item) on anyone but a hybrid champion (Guinsoo's Rageblade, Statikk Shiv and Sword of Blossoming Dawn only get built on marksmen).
- additionally, supports get a World Atlas item in the first slot: Solstice Sleigh for tanks whose kit immobilizes for a second or more in total, Celestial Opposition for other tanks, Dream Maker for supports that heal, shield or buff allies, Zaz'Zak's Realmspike for mages and other AP supports, and Bloodsong for the rest. Chef and Monk are exceptions and get Dream Maker despite being tagged as Tank.  

Items you pin are never overridden, and the AI works around them. Switch to [b]Allow Any Builds[/b] to let the AI roam free.  

[h1] Item Stats [/h1]
The Statistics screen's [b]Item Stats[/b] tab shows each item's games, wins, losses, win rate, pick rate and first-item rate for your save. Sort by any column, and filter by class, tier or lane.  

Works alongside [b]Bows' Drafter's Toolbox[/b]. With both enabled, its Advanced Stats page becomes a fifth tab in the same bar.  

[h2] Credits [/h2]
Thank you to [b]@SUB[/b] from the Korean modding community for your help with the updated item builds hook and for allowing me to integrate the 4 item mode mod into this one, which carried the fourth item slot until the base game added its own in 0.6.0.

Special thanks to all the playtesters/modders that helped me out along the way by offering ideas, sending crash dumps and testing beta builds:
[b]@toxicsnek[/b], [b]@Monsoon[/b], [b]@blasé[/b], [b]@Guardsman C[/b], [b]@kmrn[/b]

Thank you to all the translators:
[b]@Formula Piggy[/b], [b]@Yuuroo[/b], [b]@GeoStelar[/b], [b]@Monsoon[/b], [b]@Dushnerd[/b], [b]@Flover[/b] 

And finally, thank you to the people in the modding discord for their help with the mod-sdk setup, documentation, and general coolness.

If you would like to support my endeavors and buy me a coffee, you can find me on ko-fi: https://ko-fi.com/shirograhm

[h2] Legalese [/h2]
This is a free fan-made mod. I am not affiliated with Riot Games in any way. Item concepts, names, and effects are all property to Riot Games.

[Code Mod Notice]
This Workshop item contains native/executable code files. Enabling it allows code to run inside the game process. Use only mods from creators you trust.
Files: riot_items_tfm2.dll
Runs on: Windows