Adds 134 new items (76 base + 58 Radiant) inspired by Riot Games (LoL/TFT/Arena) to Teamfight Manager 2.  
Also re-skins the 30 existing items and adds some custom icons for Armor Penetration, Magic Penetration, Cooldown Reduction, Tenacity, Omnivamp, and Skill Damage Reduction.  

[b]This mod supports custom item values, custom item builds, and a way to add another item slot. See instructions below![/b]  

Lastly, this mod also offers the ability to force unique item builds. Duplicates chosen by the AI are swapped for another item of the same category, 4th item included. Toggle it in the in-game Build Editor.  

[h1] Important [/h1]
This mod currently supports English, Vietnamese, Portuguese (BR), Russian, Chinese (Simplified), and Korean locales. You can use it with other languages, but the names and descriptions of the items will be broken.  

If you would like to provide translations, feel free to shoot me a message on Discord @shirograhm.  

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.  

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Known Issues [/h1]

Some AI champions seem to prefer the wrong stats when given deference of item selection.  

The SoloQ page may sometimes show incorrect item builds.  

The item build editor currently only supports EN language for item and champion names.  

Older saves that were using older version of this mod may experience lag spikes during the BP phase and in-game. This is being investigated, for now, current workaround for this is to use a new save.  

[h1] Instructions [/h1]
If you are only seeing Bloodthirster/Luden's/Sunfire (vanilla items, no modded), that means the save you are playing is not loading the mod order. To fix this, try the following:

1. Save your current game and go back to the main menu.
2. Click Load -> Load on the save you just made. It will probably say "No Info" under mods.
2. Once launched, progress once and save again. Then go back to the main menu.
3. Click Load -> Load again, this time the mods column should read "Match". <-- (That means it's loading correctly)
4. Play as you would normally!

Currently updated for game version 0.5.7. Also supports previous versions using an older build of the mod. See below for reference:  
Mod v0.9.3+ - v0.5.7
Mod v0.9.2 - v0.5.6
Mod v0.9.0-1 - v0.5.5
Mod v0.8.0+ - v0.5.4
Mod v0.7.0+ - v0.5.3
Mod v0.5.7+ - v0.5.1 & v0.5.2
Mod v0.5.6 - versions up to 0.5.0

[h1] Custom Item Values [/h1]
This mod works directly out of the box!  

However, if any of the modded items feel too strong/weak, this mod supports full customization on all modded item values. To do so:

1. Make a copy of the [b]config-default.json[/b] that ships with this mod, and name it [b]config.json[/b]. [i]Make sure to name it exactly or else this will not work.[/i]
2. Edit the new [b]config.json[/b] with the custom values that you want.
3. Run [b]apply_config.bat[/b] to auto-generate the item effect text with the new values. If you don't do this, the mod will still use your custom values, but the item effect's text may not match.  
4. Re-run the game and open your save. No need to disable/re-enable the mod if you already had it running in the save!

Your config.json is your item information save. If you lose it, you can re-copy the default values from config-default.json. Otherwise, the game will run with the default hardcoded values.

Both files (config-default & apply_config.bat) should be located in the mod's workshop folder in your SteamLibrary: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]

[h1] Custom Item Builds [/h1]
This mod lets you override the in-game Item Strategy Screen and choose any item in the game. It is all done in-game, no external tools:

1. After draft, on the Item Strategy Screen, click [b]Builds[/b] at the top.
2. Press [b]+ Add Champion[/b], pick a champion, then set its item slots.
  a. Modded champions are listed too, so long as their mod is loaded.
  b. If you only want to decide [i]some[/i] items, set the slots you care about and leave the rest on [b]Let Player Decide[/b] (the default). The mod keeps your chosen items and lets the game's AI fill the remaining slots.
  c. Use the [b]filter by champion[/b] box in the toolbar to find a champion once the list gets long.
3. Start the simulated match!

Builds are saved automatically when changed to [b]item-builds.json[/b] as you make them, so they carry across sessions. Click [b]Save Item Builds[/b] to run a manual save.  

[h1] 4 Item Mode [/h1]
This mod can be used with a 4th item slot. To enable/disable this slot:

1. In the mod's folder, locate the file [b]4items.cfg[/b] and open it with your preferred text editor.
2. Set [b]slots = 3[/b] (disable) or [b]slots = 4[/b] (enable) and save. Then restart your game.

Next time you play a match, you should notice a 4th item slot both in your Builds editor and in the game sim.  

[h1] Planned Features [/h1]
- More support items (Ardent Censer?)
- Ability to edit values on the 30 base game items, in addition to the current config.json.
- Translations for other locales.
- Various bugfixes.

Check the Item/Feature Requests forum in the mod for the most up-to-date information.

[h2] Credits [/h2]
Thank you to @SUB from the Korean modding community for your help with the updated item builds hook and for allowing me to integrate the 4 item mode mod into this one.

Thank you to @Formula Piggy and @Yuuroo on discord for Vietnamese translations!
Thank you to @GeoStelar on discord for the Portuguese (BR) translations!
Thank you to @Monsoon on discord for the Chinese (Simplified) translations!
Thank you to @Dushnerd on discord for the Russian translations!
Thank you to @Flover on discord for the Korean translations!

Special thanks to @Monsoon for helping with the custom item builds functionality!  
Special thanks to @blasé for helping playtest the 4 Item Mod compatibility!  

Thank you to the people in the modding discord for their help with the mod-sdk setup, documentation, and general coolness.

[h2] Legalese [/h2]
This is a free fan-made mod. I am not affiliated with Riot Games in any way. Item concepts, names, and effects are all property to Riot Games.

[Code Mod Notice]
This Workshop item contains native/executable code files. Enabling it allows code to run inside the game process. Use only mods from creators you trust.
Files: apply_config.bat, apply_config.ps1, riot_items_tfm2.dll
Runs on: Windows