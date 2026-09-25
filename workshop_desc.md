Adds 6 item slots & 190 new items (112 base + 78 Radiant) to Teamfight Manager 2.  
Also re-skins the 30 existing items and adds custom icons for Armor Penetration, Magic Penetration, Ability Haste, Tenacity, Omnivamp, and Skill Damage Reduction.  

[b]Supports custom item values, custom item builds, and Smart Builds for the AI. See below![/b]  

[h1] Important [/h1]
This mod currently supports English, Vietnamese, Portuguese (BR), Russian, Chinese (Simplified), and Korean locales. You can use it with other languages, but the names and descriptions of the items will be broken.  

If you would like to provide translations, feel free to shoot me a message on Discord @shirograhm.  

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.  

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Known Issues [/h1]
- Some AI champions prefer the wrong stats when picking their own items. Keep [b]Enforce Smart Builds[/b] on to prevent this.  
- The SoloQ page may sometimes show incorrect item builds.  
- The build editor only shows item and champion names in English.  
- Saves from older versions of this mod may lag during the BP phase and in-game. For now, use a new save.  
- Largely untested in multiplayer. It should work, but custom item builds only follow the host's choices.  

[h1] Instructions [/h1]
If you are only seeing Bloodthirster/Luden's/Sunfire (vanilla items, no modded), that means the save you are playing is not loading the mod order. To fix this, try the following:

1. Save your current game and go back to the main menu.
2. Click Load -> Load on the save you just made. It will probably say "No Info" under mods.
2. Once launched, progress once and save again. Then go back to the main menu.
3. Click Load -> Load again, this time the mods column should read "Match". <-- (That means it's loading correctly)
4. Play as you would normally!

[h1] Versioning [/h1]
Currently updated for game version 0.6.1. Older game versions need an older build of the mod:  
Mod v0.10.1+ - 0.6.1  
Mod v0.9.11-0.10.0 - 0.6.0  
Mod v0.9.6-9 - v0.5.8  

Manual releases: https://github.com/shirograhm/riot-items-tfm2/releases

[h1] Custom Item Values [/h1]
This mod works directly out of the box!  

However, if any of the items feel too strong/weak, this mod supports full customization on all item values. To do so:  

1. Make a copy of the [b]config-default.json[/b] that ships with this mod, and name it [b]config.json[/b]. [i]Make sure to name it exactly or else this will not work.[/i]  
2. Edit the new [b]config.json[/b] with the custom values that you want.  
3. Run [b]apply_config.bat[/b] to auto-generate the item effect text with the new values. If you don't do this, the mod may not update the values correctly.  
4. Re-run the game and open your save. No need to disable/re-enable the mod if you already had it running in the save!  

Your config.json is your item information save. If you lose it, you can re-copy the default values from config-default.json. Otherwise, the game will run with the default hardcoded values.  

Both files (config-default & apply_config.bat) should be located in the mod's workshop folder in your SteamLibrary: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]

[h1] Custom Item Builds [/h1]
Pick any item for any champion, in-game:
1. After draft, on the Item Strategy Screen, click [b]Builds[/b] at the top.  
2. Press [b]+ Add Champion[/b], pick a champion (modded champions included), and set its item slots. Any slot left on [b]Let Player Decide (-)[/b] is filled by the AI.  
3. Start the match!  

Use the [b]filter by champion[/b] box to find champions in a long list (separate several with commas). Builds save automatically to [b]item-builds.json[/b] and carry across sessions. [b]Save Item Builds[/b] saves manually.  

[h1] Smart Builds [/h1]
[b]Enforce Smart Builds[/b] (Build Editor footer, on by default) cleans up the AI's picks. These get swapped for another item of the same category:
- duplicates, a second Grievous Wounds item, or crit past 100% (passive crit counts as fully stacked)
- 1 boots item per player in the second slot (unless pinned elsewhere in the build)
- support items (except Protoplasm Harness) outside the support role
- items the champion doesn't scale with: attack-only items on an AP champion, or AP-only items on an AD champion (hybrids are left alone). Champions from other mods are covered too.

It also sets the buy order: items that get stronger the longer you own them come first, and items that scale off the rest of the build come last.  

Items you pin are never overridden, and the AI works around them. Switch to [b]Allow Any Builds[/b] to let the AI roam free.  

[h1] Item Stats [/h1]
The Statistics screen's [b]Item Stats[/b] tab shows each item's games, wins, losses, win rate, pick rate and first-item rate for your save. Sort by any column, and filter by class, tier or lane.  

[h1] Planned Features [/h1]
- More support items.
- Translations for other locales.
- Various bugfixes.

Check the Item/Feature Requests forum in the mod for the most up-to-date information.

[h2] Credits [/h2]
Thank you to [b]@SUB[/b] from the Korean modding community for your help with the updated item builds hook and for allowing me to integrate the 4 item mode mod into this one, which carried the fourth item slot until the base game added its own in 0.6.0.

Special thanks to all the playtesters that helped me out along the way by sending me crash dumps and testing beta builds:
[b]@toxicsnek[/b] for helping with custom item creation & code!
[b]@Monsoon[/b] for helping with the custom item builds functionality!  
[b]@blasé[/b] for helping playtest the 4 Item Mode compatibility!  
[b]@Guardsman C[/b] & [b]@kmrn[/b] for helping playtest the new item hooks on 0.6.0!

Thank you to [b]@Formula Piggy[/b] & [b]@Yuuroo[/b] on discord for Vietnamese translations!
Thank you to [b]@GeoStelar[/b] on discord for the Portuguese (BR) translations!
Thank you to [b]@Monsoon[/b] on discord for the Chinese (Simplified) translations!
Thank you to [b]@Dushnerd[/b] on discord for the Russian translations!
Thank you to [b]@Flover[/b] on discord for the Korean translations!

And finally, thank you to the people in the modding discord for their help with the mod-sdk setup, documentation, and general coolness.

If you would like to support my endeavors and buy me a coffee, you can find me on ko-fi: https://ko-fi.com/shirograhm

[h2] Legalese [/h2]
This is a free fan-made mod. I am not affiliated with Riot Games in any way. Item concepts, names, and effects are all property to Riot Games.

[Code Mod Notice]
This Workshop item contains native/executable code files. Enabling it allows code to run inside the game process. Use only mods from creators you trust.
Files: apply_config.bat, apply_config.ps1, riot_items_tfm2.dll
Runs on: Windows
