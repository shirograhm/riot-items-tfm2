Adds 6 item slots & 239 new items (139 base + 100 Radiant) to Teamfight Manager 2.  
Also re-skins the 30 existing items and adds custom icons for Armor Penetration, Magic Penetration, Ability Haste, Tenacity, Omnivamp, and Skill Damage Reduction.  
Champions start each match with 250 gold instead of 500.  

[b]Supports custom item values, custom item builds, and Smart Builds for the AI. See below![/b]  

[h1] Important [/h1]
This mod currently supports English, Vietnamese, Portuguese (BR), Russian, Chinese (Simplified), and Korean locales. You can use it with other languages, but the names and descriptions of the items will be broken.  

If you would like to provide translations, feel free to shoot me a message on Discord @shirograhm.  

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.  

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Versioning [/h1]
Currently updated for game version 0.6.4. Older game versions need an older build of the mod:  
Mod v1.0.1+ - 0.6.4  
Mod v0.11.11-1.0.0 - 0.6.3  
Mod v0.11.3-0.11.10 - 0.6.2  
Mod v0.10.1-0.11.2 - 0.6.1  
Mod v0.9.11-0.10.0 - 0.6.0  

Manual releases (includes beta releases): https://github.com/shirograhm/riot-items-tfm2/releases

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
- Largely untested in multiplayer. It may work, but custom item builds only follow the host's choices.  

[h1] Custom Item Values [/h1]
This mod works directly out of the box! However, if any of the items feel too strong/weak, this mod supports full customization on all item values. To do so:  

1. Make a copy of the [b]config-default.json[/b] that ships with this mod, and name it [b]config.json[/b]. [i]Make sure to name it exactly or else this will not work.[/i]  
2. Edit the new [b]config.json[/b] with the custom values that you want.  
3. Re-run the game and open your save. No need to disable/re-enable the mod if you already had it running in the save!  

Your config.json is your item information save. If you lose it, you can re-copy the default values from config-default.json. Otherwise, the game will run with the default hardcoded values.  

Both files (config-default.json & config.json) should be located in the mod's workshop folder in your SteamLibrary: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]

[h1] Custom Item Builds [/h1]
Pick any item for any champion, in-game:
1. After draft, on the Item Strategy Screen, click [b]Builds[/b] at the top.  
2. Press [b]+ Add Champion[/b], pick a champion (modded champions included), and set its item slots. Any slot left on [b]Let Player Decide (-)[/b] is filled by the AI.  
3. Start the match!  

Use the [b]filter by champion[/b] box to find champions in a long list (separate several with commas). After a draft, [b]Blue Team[/b] and [b]Red Team[/b] under the Matchup card fill the box with that side's champions. Builds save automatically to [b]item-builds.json[/b] and carry across sessions. [b]Save Item Builds[/b] saves manually.  

During a match, you can see both sides' planned builds using the [b]Check Tactics[/b] tab. Pinned items will render with a teal border.  

[h1] Item Patches [/h1]
Whenever the game patches its champions, this mod patches its items too: strong items are nerfed, weak ones buffed. Changes arrive in your inbox as [b]Item Patch Notes[/b]. To set numbers yourself, open [b]Item Patcher Settings[/b] under [b]Options > Competition[/b], type new values, and click [b]Queue Manual Patch[/b].  

To tune the patch system, make a copy of the [b]config-autopatch-default.json[/b] that ships with this mod (same folder as config-default.json), and name it [b]config-autopatch.json[/b]. Re-run the game after editing it. Without a rename, your numbers could be overwritten on update.  

[h1] Smart Builds [/h1]
[b]Enforce Smart Builds[/b] cleans up the AI's picks using the following rules:
1. No duplicate items  
2. One Grievous Wounds item  
3. Crit capped at 100%, incl. passives  
4. Support items only on supports  
5. Items match the champion's scaling (AD/AP/hybrid)  
6. Stacking items only in the first two slots, scaling items only from the third on  
7. One pair of boots  
8. One jungle item, junglers only  
9. Role items bought first, a jungler's jungle item in slot 1 (none if you pin slot 1)  
10. Imperial Mandate on supports that immobilize  
11. Melee/ranged items match the champion's reach  
12. A World Atlas item in slot 1 of support builds (none if you pin slot 1)  
13. Heal/shield/buff items for supports that aid allies  
14. Guinsoo's, Statikk Shiv and Sword of Blossoming Dawn for marksmen characters only  
15. One Spellblade item  
16. Tank supports build only Tank/Support items  
17. Heal/shield/buff supports build only Tank/Support items, with Ardent/Helia/Staff prioritized  

Items you pin are never overridden, and the AI works around them. Switch to [b]Allow Any Builds[/b] to let the AI roam free.  

[h1] Item Stats [/h1]
The Statistics screen's [b]Item Stats[/b] tab shows each item's games, wins, losses, win rate, pick rate and first-item rate for your save. Sort by any column, and filter by class, tier or lane.  

Works alongside [b]Bows' Drafter's Toolbox[/b]. With both enabled, its Advanced Stats page becomes a fifth tab in the same bar.  

[h2] Credits [/h2]
Thank you to the modders that helped me with code and feature implementation:  
[b]@SUB[/b] and [b]@toxicsnek[/b]  

Special thanks to all the playtesters that helped me out along the way by sending crash dumps and testing beta builds:  
[b]@Monsoon[/b], [b]@blasé[/b], [b]@Guardsman C[/b], [b]@kmrn[/b]

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