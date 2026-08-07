Adds 128 new items (73 base + 55 Radiant) inspired by Riot Games (LoL/TFT/Arena) to Teamfight Manager 2.  
Also re-skins the 30 existing items and adds some custom icons for Armor Penetration, Magic Penetration, Cooldown Reduction, Tenacity, Omnivamp, and Skill Damage Reduction.  

[b]This mod supports custom item values, custom item builds, and a way to add another item slot. See instructions below![/b]  

Lastly, this mod also offers the ability to force unique item builds. Duplicates chosen by the AI are swapped for an item of the same category. Toggle it in the in-game Build Editor.  

[h1] Important [/h1]

This mod currently supports English, Vietnamese, Portuguese (BR), Russian, Chinese (Simplified), and Korean locales. You can use it with other languages, but the names and descriptions of the items will be broken.

If you would like to provide translations, feel free to shoot me a message on Discord @shirograhm.

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Known Issues [/h1]

Some item effects are roughly simulated to the best of my ability using the available mod-sdk.  

Some AI champions seem to prefer the wrong stats when given deference of item selection.  

Some AI champions ignore the custom item builds. Likely to be instability that occurs in long term builds.  

This mod can cause your game to crash if you back out to the main menu and then attempt to load the game again. This is a side-effect of having 4 items mode enabled.  

[h1] Instructions [/h1]

In order for the AI to realize that these items exist, it is best to add this mod to an existing save instead of creating a new one. Try the following:

1. Subscribe to this modpack in the Steam workshop.
2. Launch Teamfight Manager 2 and create a new game with the mod disabled.
3. Save and exit back to the main menu, then enable the mod.
4. Restart the game.
5. Continue your saved game, and proceed through the mod mismatch popup.

Currently updated for game version >=0.5.3. Also supports previous versions using an older build of the mod. See below for reference:  
Mod v0.8.0+ - v0.5.4+
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
3. Start the simulated match!

Builds are saved to [b]item-builds.json[/b] as you make them, so they carry across sessions.  

[b]NOTE: Custom item builds override all matches at the moment. So your build becomes the meta. It is extremely powerful and can affect both simulated games, soloQ, and even the other team's champions in your match. Please use it with caution![/b]

[h1] 4 Item Mode [/h1]

This mod can be used with a 4th item slot. To enable this slot:

1. In the mod's folder, locate the file [b]4items.cfg[/b] and open it with your preferred text editor.
2. Set [b]slots = 4[/b] and save. Then restart your game.

Next time you play a match, you should notice a 4th item slot both in your Builds editor and in the game sim.  

[h1] Planned Features [/h1]

- Support items (Ardent Censer?) and possibly a 7th item category?
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