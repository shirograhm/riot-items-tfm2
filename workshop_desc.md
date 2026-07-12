Adds 97 new items (55 base + 42 Radiant) inspired by Riot Games (LoL/TFT/Arena) to Teamfight Manager 2.  

Also re-skins the 30 existing items and adds some custom icons for Armor Penetration, Magic Penetration, Cooldown Reduction, Tenacity, Omnivamp, and Skill Damage Reduction.  

[b]This mod supports custom item values! Now you can be Phreak! See the instructions below.[/b]  
[b]This mod also supports custom item builds! See the instructions below.[/b]  

[h1] Important [/h1]

This mod currently supports English, Vietnamese, Portuguese (BR), Russian, and Chinese (Simplified) locales. You can use it with other languages, but the names and descriptions of the items will be broken.

If you would like to provide translations, feel free to shoot me a message on Discord @shirograhm.

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Known Issues [/h1]

Selecting a specific item category on the Item Selection Screen forces the AI to buy the base game item in that category. See the [b]Custom Item Builds[/b] section below if you want to control the items your players build!

The item build override affects all champions in the match. This means you could control the enemy's items. Currently there is no fix without removing the custom item build behavior completely.  

Some item effects (on-hit damage, healing) may not show the damage numbers. Not sure why that is at this point. However, the effects are working.  

Some item effects are roughly simulated to the best of my ability using the available mod-sdk.

Some AI champions seem to prefer the wrong stats when given deference of item selection. Magic Knight tends to buy AD and AS items instead of AP, for example.  

Some AI champions ignore the custom item build editor. This is being investigated, but is likely to be instability that occurs in long term builds.  

[h1] Instructions [/h1]

In order for the AI to realize that these items exist, it is best to add this mod to an existing save instead of creating a new one. Try the following:

1. Subscribe to this modpack in the Steam workshop.
2. Launch Teamfight Manager 2 and create a new game with the mod disabled.
3. Save and exit back to the main menu, then enable the mod.
4. Restart the game.
5. Continue your saved game, and proceed through the mod mismatch popup.

Currently updated for game version ^0.4.12.

[h1] Custom Item Values [/h1]

This mod works directly out of the box!  

However, if any of the modded items feel too strong/weak, this mod supports full customization on all item values. Only modded items can be adjusted. To do so:

1. Make a copy of the [b]config-default.json[/b] that ships with this mod, and name it [b]config.json[/b]. [i]Make sure to name it exactly or else this will not work.[/i]
2. Edit the new [b]config.json[/b] with the custom values that you want.
3. Run [b]apply_config.bat[/b] to auto-generate the item effect text with the new values. If you don't do this, the mod will still use your custom values, but the item effect's text may not match.  
4. Re-run the game and open your save. No need to disable/re-enable the mod if you already had it running in the save!

Your config.json is your item information save. If you lose it, you can re-copy the default values from config-default.json. Otherwise, the game will run with the default hardcoded values.

Both files (config-default & apply_config.bat) should be located in the mod's workshop folder in your SteamLibrary: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]

[h1] Custom Item Builds [/h1]

This mod allows for the user to override the current in-game Item Strategy Screen to choose any item in the game. To do so:

1. Navigate to the SteamLibrary workshop folder: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]
2. Run [b]item-build-editor.bat[/b] to open the editor GUI. Keep the [b]Item Build Editor[/b] GUI open while your game is running.
3. After draft, while on the Item Strategy Screen, add champions to the GUI and set up your builds.
  a. This also supports modded champions, given you know the champion ID. For Silverbear's mod, use names formatted like [b]"test_mod_vayne"[/b].
4. Start the simulated match!

[b]NOTE: Custom item builds override all matches at the moment. So your build becomes the meta. It is extremely powerful and can affect both simulated games, soloQ, and even the other team's champions in your match. Please use it with caution![/b]

[h1] Planned Features [/h1]

- Support items (Ardent Censer?) and possibly a 7th item category?
- Ability for the user to select specific items on each champion in the post-draft screen. [i]Currently works with a side-loaded GUI.[/i]
- Make items unique purchases (one per champion) to prevent effects from stacking.
- Ability to edit values on the 30 base game items, in addition to the current config.json.
- Translations for other locales.
- Various bugfixes.

Check the Item/Feature Requests forum in the mod for the most up-to-date information.

[h2] Credits [/h2]

Thank you to @Formula Piggy and @Yuuroo on discord for Vietnamese translations!
Thank you to @GeoStelar on discord for the Portuguese (BR) translations!
Thank you to @Monsoon on discord for the Chinese (Simplified) translations!
Thank you to @Dushnerd on discord for the Russian translations!

Special thanks to @Monsoon for helping with the custom item builds functionality!

Thank you to the people in the modding discord for their help with the mod-sdk setup, documentation, and general coolness.

[h2] Legalese [/h2]

This is a free fan-made mod. I am not affiliated with Riot Games in any way. Item concepts, names, and effects are all property to Riot Games.

[Code Mod Notice]
This Workshop item contains native/executable code files. Enabling it allows code to run inside the game process. Use only mods from creators you trust.
Files: apply_config.bat, apply_config.ps1, item-build-editor.bat, item-build-editor.ps1, riot_items_tfm2.dll