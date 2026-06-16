Adds items inspired from Riot Games (LoL/TFT/Arena) to Teamfight Manager 2.  
Re-skins the 30 existing items and also adds 39 new items (18 new item lines) to the game.  

Also adds some custom icons for Armor Penetration, Magic Penetration, Cooldown Reduction, Tenacity, Omnivamp, and Skill Damage Reduction.  

[b]This mod supports custom item values! Now you can be Phreak! See the instructions below.[/b]  

[h1] Important [/h1]

This mod currently supports both English and Vietnamese locales. You can use it with other languages, but the names and descriptions of the items will be broken.

If you would like to provide translations, please, shoot me a message on Discord @shirograhm.

Saves played with this mod enabled will be corrupted if you play the save with this mod disabled. I have had players buy ghost items when this happens.

[h3][i] THIS MOD WILL CHANGE THE BALANCE OF YOUR GAME. USE WITH CAUTION. [/i][/h3]

[h1] Known Issues [/h1]

Selecting a specific item category for the AI to buy will cause it to always buy the base game item. [i][b]To get maximum use out of this mod, you'll have to trust your players![/b][/i]  

Some item effects (on-hit damage, healing) may not show the damage numbers. Not sure why that is at this point. However, the effects should be working.

Some AI champions seem to prefer the wrong stats. Magic Knight tends to buy AD and AS items instead of AP, for example.

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
1. Edit the new [b]config.json[/b] with the custom values that you want.
2. Run [b]apply_config.bat[/b] to auto-generate the item effect text with the new values. If you don't do this, the mod will still use your custom values, but the item effect's text may not match.  
3. Re-run the game and open your save. No need to disable/re-enable the mod if you already had it running in the save!

Your config.json is your item information save. If you lose it, you can re-copy the default values from config-default.json. Otherwise, the game will run with the default hardcoded values.

Both files should be located in the mod's workshop folder in your SteamLibrary: [b]SteamLibrary/steamapps/workshop/content/3009300/3739568852/[/b]

[h1] Planned Features [/h1]

- Support items (Ardent Censer?) and possibly a 7th item category?
- Ability for the user to select specific items on each champion in the post-draft screen.
- Make items unique purchases (one per champion) to prevent effects from stacking.
- Ability to edit values on the 30 base game items, in addition to the current config.json.
- Translations for other locales.
- Various bugfixes.

Check the Item/Feature Requests forum in the mod for the most up-to-date information.

[h2] Credits [/h2]

Thank you to @Formula Piggy and @Yuuroo in the discord for Vietnamese translations!
Thank you to the discord for their help with the mod-sdk setup, documentation, and general coolness.

[h2] Legalese [/h2]

This is a free fan-made mod. I am not affiliated with Riot Games in any way. Item concepts, names, and effects are all property to Riot Games.

[Code Mod Notice]
This Workshop item contains native/executable code files. Enabling it allows code to run inside the game process. Use only mods from creators you trust.
Files: apply_config.bat, apply_config.ps1, riot_items_tfm2.dll