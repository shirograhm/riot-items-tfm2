#requires -version 5.1
<#
  Item Build Editor (native WinForms GUI) for riot_items_tfm2.

  One row per champion: champion dropdown + 3 item dropdowns. Reads and writes
  item-builds.json in this script's own folder (so drop this .ps1 next to it).

  - Auto-loads item-builds.json from the script folder on start.
  - Autosaves to that file on every change (the Save button forces a save too).
  - Pick a champion, or choose "(modded champion)" to type a raw/modded id.

  Launch via item-build-editor.bat, or:
    powershell -STA -ExecutionPolicy Bypass -File item-build-editor.ps1
#>

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

# EM_SETCUEBANNER lets us show placeholder text in a TextBox on .NET Framework.
Add-Type -Namespace Native -Name Edit -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern int SendMessage(System.IntPtr hWnd, int msg, int wParam, string lParam);
'@

$CHAMPIONS = @(
  'android', 'alchemist', 'archer', 'bard', 'barrier_magician', 'berserker', 'bomber',
  'boomerang_hunter', 'cavalry_knight', 'chef', 'circus_blade', 'clown', 'crossbowman', 'dancer',
  'dark_mage', 'demon', 'dokkaebi', 'druid', 'dual_blader', 'enchanter',
  'executioner', 'exorcist', 'fighter', 'gambler', 'ghost', 'guardian_spirit',
  'gunner', 'hammerer', 'hitman', 'hunter', 'ice_mage', 'illusionist',
  'inquisitor', 'jiangshi', 'knight', 'lancer', 'lightning_mage', 'magic_knight',
  'monk', 'necromancer', 'nightmare', 'ninja', 'ogre', 'plague_doctor', 'poison_dart_hunter',
  'pole_warrior', 'priest', 'prisoner', 'pyromancer', 'pythoness', 'sand_mage', 'shadowmancer',
  'shield_bearer', 'siege_breaker', 'soldier', 'spirit_caller', 'swordman', 'taoist',
  'vampire', 'voodoo_shaman', 'werewolf', 'whip_master', 'white_mage', 'wind_mage'
)
$ITEMS = @(
  'atmas_reckoning', 'bastionbreaker', 'black_cleaver', 'blackfire_torch', 'blade_of_the_ruined_king', 'bloodletters_curse', 'bloodsong', 'bloodthirster', 'collector',
  'deathblade', 'deaths_dance', 'diamond_tipped_spear', 'dragons_claw', 'dusk_and_dawn', 'echoes_of_helia', 'experimental_hexplate', 'frozen_mallet',
  'guinsoos_rageblade', 'heartsteel', 'hextech_gunblade', 'hubris', 'infinity_edge', 'jaksho_the_protean',
  'kraken_slayer', 'liandrys_torment', 'lord_dominiks_regards', 'ludens_tempest', 'malignance', 'mirage_blade', 'morellonomicon', 'mortal_reminder', 'nashors_tooth',
  'night_harvester', 'overlords_bloodmail', 'phantom_dancer',
  'protectors_vow', 'protoplasm_harness', 'rabadons_deathcap', 'riftmaker', 'rylais_crystal_scepter', 'serpents_fang', 'shadowflame', 'spear_of_shojin',
  'spirit_visage', 'stormrazor', 'sundered_sky', 'sunfire_cape', 'terminus', 'thornmail', 'trinity_force', 'unending_despair',
  'void_staff', 'warmogs_armor', 'wits_end', 'yun_tal_wildarrows', 'zekes_herald'
)

$CHAMP_PLACEHOLDER = '(champion)'
# The default item slot: any slot you don't set is left for the game's AI to fill.
# It is written to item-builds.json as JSON null; the mod keeps your pinned items
# and lets the AI choose every blank slot.
$ITEM_PLACEHOLDER = '(AI picks)'
$AI_SENTINEL = '__AI__'   # internal id for an (AI picks) slot; written as JSON null
$MODDED_LABEL = '(modded champion)'

function Format-Name($id) {
  if ($id -eq 'soldier') { return 'Soldier (Sniper)' }
  ($id -split '_' | ForEach-Object {
    if ($_.Length) { $_.Substring(0, 1).ToUpper() + $_.Substring(1) }
  }) -join ' '
}

# display <-> id maps
$script:ChampDisplay = @($CHAMP_PLACEHOLDER, $MODDED_LABEL) + ($CHAMPIONS | ForEach-Object { Format-Name $_ })
# $script:ItemDisplay is built later (grouped by item category), once $scriptDir is known.
$script:ChampToId = @{ $CHAMP_PLACEHOLDER = '' }
$script:ChampToDisplay = @{}
foreach ($c in $CHAMPIONS) { $d = Format-Name $c; $script:ChampToId[$d] = $c; $script:ChampToDisplay[$c] = $d }
$script:ItemToId = @{ $ITEM_PLACEHOLDER = $AI_SENTINEL }
$script:ItemToDisplay = @{ $AI_SENTINEL = $ITEM_PLACEHOLDER }
foreach ($it in $ITEMS) { $d = Format-Name $it; $script:ItemToId[$d] = $it; $script:ItemToDisplay[$it] = $d }

# --- colors ----------------------------------------------------------------
$cBg = [System.Drawing.Color]::FromArgb(22, 23, 33)
$cPanel = [System.Drawing.Color]::FromArgb(29, 31, 44)
$cPanel2 = [System.Drawing.Color]::FromArgb(15, 16, 22)
$cText = [System.Drawing.Color]::FromArgb(232, 232, 232)
$cMuted = [System.Drawing.Color]::FromArgb(165, 165, 171)
$cAccent = [System.Drawing.Color]::FromArgb(96, 221, 194)
$cDanger = [System.Drawing.Color]::FromArgb(232, 100, 90)
$cBadRow = [System.Drawing.Color]::FromArgb(58, 31, 34)
$cDragRow = [System.Drawing.Color]::FromArgb(28, 50, 58)  # row being dragged

# column geometry (x, width)
$COL = @{
  Grip  = @(0, 24)
  Champ = @(30, 275)
  I1    = @(318, 200)
  Swap1 = @(521, 30)
  I2    = @(554, 200)
  Swap2 = @(757, 30)
  I3    = @(790, 200)
  Del   = @(1003, 35)
}

# --- target file -----------------------------------------------------------
$scriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
$script:TargetPath = Join-Path $scriptDir 'item-builds.json'
$script:SettingsPath = Join-Path $scriptDir 'mod-settings.json'

# Behavior toggles shared with the mod DLL, which re-reads mod-settings.json on
# every match start - so flipping the checkbox applies to the next match without
# restarting the game. unique_items: when true (the default, also used when the
# file is absent) the mod rewrites builds so no champion ever holds duplicate
# copies of an item; unchecking allows duplicate builds again.
$script:UniqueEnabled = $true
if (Test-Path -LiteralPath $script:SettingsPath) {
  try {
    $settings = Get-Content -Raw -LiteralPath $script:SettingsPath -Encoding UTF8 | ConvertFrom-Json
    if ($null -ne $settings.unique_items) { $script:UniqueEnabled = [bool]$settings.unique_items }
  }
  catch {}
}

# --- item category grouping ------------------------------------------------
# The item dropdowns are grouped under category headers. Categories are a manual
# map ($itemCategory: item id -> category key) maintained below - edit it to move
# an item between groups. Any item in $ITEMS not listed there falls under a
# trailing "Other" header so nothing ever disappears from the picker.
$script:ItemHeaderIndices = New-Object 'System.Collections.Generic.HashSet[int]'
$script:GroupItems = $true

# Category header rows, in dropdown order: internal category key -> display label.
$CATEGORY_ORDER = @(
  @('Assassin', 'Assassin'), @('Fighter', 'Fighter'), @('Tank', 'Tank'), 
  @('Mage', 'Mage'), @('Marksman', 'Marksman'), @('Support', 'Support')
)
# Item id -> category key. Keys must match ids in $ITEMS.
$itemCategory = @{
  # Assassin
  bastionbreaker           = 'Assassin';
  collector                = 'Assassin';
  hubris                   = 'Assassin';
  serpents_fang            = 'Assassin';

  # Fighter
  black_cleaver            = 'Fighter'; 
  bloodthirster            = 'Fighter';
  experimental_hexplate    = 'Fighter';
  deaths_dance             = 'Fighter'; 
  frozen_mallet            = 'Fighter';
  overlords_bloodmail      = 'Fighter';
  spear_of_shojin          = 'Fighter';
  sundered_sky             = 'Fighter';
  trinity_force            = 'Fighter';

  # Marksman
  blade_of_the_ruined_king = 'Marksman';
  deathblade               = 'Marksman'; 
  diamond_tipped_spear     = 'Marksman';
  guinsoos_rageblade       = 'Marksman'; 
  infinity_edge            = 'Marksman'; 
  kraken_slayer            = 'Marksman'; 
  lord_dominiks_regards    = 'Marksman'; 
  mirage_blade             = 'Marksman';
  mortal_reminder          = 'Marksman';
  phantom_dancer           = 'Marksman'; 
  stormrazor               = 'Marksman'; 
  terminus                 = 'Marksman';
  wits_end                 = 'Marksman';
  yun_tal_wildarrows       = 'Marksman';

  # Mage
  blackfire_torch          = 'Mage';
  bloodletters_curse       = 'Mage';
  dusk_and_dawn            = 'Mage';
  hextech_gunblade         = 'Mage';
  liandrys_torment         = 'Mage';
  ludens_tempest           = 'Mage';
  malignance               = 'Mage'; 
  morellonomicon           = 'Mage';
  nashors_tooth            = 'Mage';
  night_harvester          = 'Mage'; 
  rabadons_deathcap        = 'Mage';
  riftmaker                = 'Mage';
  rylais_crystal_scepter   = 'Mage';
  shadowflame              = 'Mage';
  void_staff               = 'Mage';
    
  # Tank
  atmas_reckoning          = 'Tank'; 
  dragons_claw             = 'Tank';
  heartsteel               = 'Tank';
  jaksho_the_protean       = 'Tank';
  protectors_vow           = 'Tank';
  spirit_visage            = 'Tank';
  sunfire_cape             = 'Tank';
  thornmail                = 'Tank';
  unending_despair         = 'Tank';
  warmogs_armor            = 'Tank';

  # Support
  bloodsong                = 'Support'; 
  echoes_of_helia          = 'Support'; 
  protoplasm_harness       = 'Support';
  zekes_herald             = 'Support';
}

# Item icons from the packed sprite sheet in aseprite_resources/ingame. The data
# file maps a frame name -> normalized rect (0..1); frames are named after the
# item id, so each mod item resolves to a source rectangle in the sheet PNG.
# Base-game reskins have no frame and just show their name. The PNG is loaded
# into an in-memory bitmap so the file isn't locked while the editor runs.
$script:IconSheet = $null
$script:IconFrames = @{}
$iconDir = Join-Path $scriptDir 'aseprite_resources\ingame'
$iconPng = Join-Path $iconDir 'item_icons_640X640#sheet.png'
$iconData = Join-Path $iconDir 'item_icons_640X640#data.sprite_sheet'
if ((Test-Path -LiteralPath $iconPng) -and (Test-Path -LiteralPath $iconData)) {
  try {
    $bytes = [System.IO.File]::ReadAllBytes($iconPng)
    $ms = New-Object System.IO.MemoryStream (, $bytes)
    $img = [System.Drawing.Image]::FromStream($ms)
    $script:IconSheet = New-Object System.Drawing.Bitmap($img)
    $img.Dispose(); $ms.Dispose()
    $sw = $script:IconSheet.Width; $sh = $script:IconSheet.Height
    $data = Get-Content -Raw -LiteralPath $iconData -Encoding UTF8 | ConvertFrom-Json
    foreach ($p in $data.images.PSObject.Properties) {
      $f = $p.Value
      $script:IconFrames[$p.Name] = New-Object System.Drawing.Rectangle(
        [int][Math]::Round($f.x * $sw), [int][Math]::Round($f.y * $sh),
        [int][Math]::Round($f.w * $sw), [int][Math]::Round($f.h * $sh))
    }
  }
  catch { $script:IconSheet = $null }
}

# Base-game reskins have no id-named frame in the sheet; map them to a specific
# frame by name. Non-reskin items resolve to the frame named after their id.
$reskinIcon = @{
  bloodthirster = 't4_0'; phantom_dancer = 't4_1'; thornmail = 't4_2'
  dragons_claw = 't4_3'; ludens_tempest = 't4_4'; sunfire_cape = 't4_5'
}
function Resolve-ItemIcon($id) {
  $frame = if ($reskinIcon.ContainsKey($id)) { $reskinIcon[$id] } else { $id }
  return $script:IconFrames[$frame]
}

# (AI picks) row first, then each non-empty category's items under a header row.
# $iconList runs parallel to $displayList: $null for placeholder/header rows, the
# sheet source rectangle for item rows that have a frame.
$displayList = [System.Collections.Generic.List[string]]::new()
$iconList = New-Object 'System.Collections.Generic.List[object]'
[void]$displayList.Add($ITEM_PLACEHOLDER); [void]$iconList.Add($null)
foreach ($grp in $CATEGORY_ORDER) {
  $inCat = @($ITEMS | Where-Object { $itemCategory[$_] -eq $grp[0] } | Sort-Object { Format-Name $_ })
  if ($inCat.Count -eq 0) { continue }
  [void]$script:ItemHeaderIndices.Add($displayList.Count)
  [void]$displayList.Add($grp[1]); [void]$iconList.Add($null)
  foreach ($it in $inCat) { [void]$displayList.Add((Format-Name $it)); [void]$iconList.Add((Resolve-ItemIcon $it)) }
}
# Any item not in $itemCategory still shows up, grouped under a trailing "Other" header.
$uncategorized = @($ITEMS | Where-Object { -not $itemCategory.ContainsKey($_) } | Sort-Object { Format-Name $_ })
if ($uncategorized.Count -gt 0) {
  [void]$script:ItemHeaderIndices.Add($displayList.Count)
  [void]$displayList.Add('Other'); [void]$iconList.Add($null)
  foreach ($it in $uncategorized) { [void]$displayList.Add((Format-Name $it)); [void]$iconList.Add((Resolve-ItemIcon $it)) }
}
$script:ItemDisplay = $displayList.ToArray()
$script:ItemIconRect = $iconList.ToArray()

# Owner-draw for the item combos: category header rows render as a muted bold
# label between two hairlines and are never highlighted; item rows highlight on
# hover/selection. Headers are identified by index via $ItemHeaderIndices.
$script:HeaderFont = New-Object System.Drawing.Font('Segoe UI', 9.75, [System.Drawing.FontStyle]::Bold)
$script:OnItemDraw = {
  param($sender, $e)
  if ($e.Index -lt 0) { return }
  $g = $e.Graphics; $b = $e.Bounds
  $text = [string]$sender.Items[$e.Index]
  if ($script:ItemHeaderIndices.Contains($e.Index)) {
    $g.FillRectangle((New-Object System.Drawing.SolidBrush($cPanel)), $b)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = 'Center'; $sf.LineAlignment = 'Center'
    $rect = New-Object System.Drawing.RectangleF($b.X, $b.Y, $b.Width, $b.Height)
    $g.DrawString($text, $script:HeaderFont, (New-Object System.Drawing.SolidBrush($cMuted)), $rect, $sf)
    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(70, 72, 86))
    $midY = $b.Y + [int]($b.Height / 2)
    $half = [int]($g.MeasureString($text, $script:HeaderFont).Width / 2)
    $cx = $b.X + [int]($b.Width / 2)
    $g.DrawLine($pen, ($b.X + 12), $midY, ($cx - $half - 8), $midY)
    $g.DrawLine($pen, ($cx + $half + 8), $midY, ($b.X + $b.Width - 12), $midY)
    return
  }
  $state = [int]$e.State
  $isEdit = (($state -band [int][System.Windows.Forms.DrawItemState]::ComboBoxEdit) -ne 0)
  $selected = (-not $isEdit) -and (($state -band [int][System.Windows.Forms.DrawItemState]::Selected) -ne 0)
  $bgCol = if ($selected) { $cAccent } else { $cPanel2 }
  $fgCol = if ($selected) { $cPanel2 } else { $cText }
  $g.FillRectangle((New-Object System.Drawing.SolidBrush($bgCol)), $b)
  # Item icon (if the item has a sheet frame) at the left; text starts after it.
  $textX = $b.X + $(if ($e.Index -eq 0) { 8 } else { 20 })
  $src = if ($script:IconSheet -and $e.Index -ge 0 -and $e.Index -lt $script:ItemIconRect.Length) { $script:ItemIconRect[$e.Index] } else { $null }
  if ($null -ne $src) {
    $sz = [Math]::Min($b.Height - 4, 22)
    $iy = $b.Y + [int](($b.Height - $sz) / 2)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
    $dest = New-Object System.Drawing.Rectangle(($b.X + 4), $iy, $sz, $sz)
    $g.DrawImage($script:IconSheet, $dest, $src.X, $src.Y, $src.Width, $src.Height, [System.Drawing.GraphicsUnit]::Pixel)
    $textX = $dest.Right + 6
  }
  $sf = New-Object System.Drawing.StringFormat
  $sf.LineAlignment = 'Center'
  $sf.FormatFlags = [System.Drawing.StringFormatFlags]::NoWrap
  $sf.Trimming = [System.Drawing.StringTrimming]::EllipsisCharacter
  $rect = New-Object System.Drawing.RectangleF($textX, $b.Y, ($b.X + $b.Width - $textX), $b.Height)
  $g.DrawString($text, $sender.Font, (New-Object System.Drawing.SolidBrush($fgCol)), $rect, $sf)
}

function Save-Settings {
  $val = if ($script:UniqueCheck.Checked) { 'true' } else { 'false' }
  $json = "{`n  `"unique_items`": $val`n}`n"
  try {
    [System.IO.File]::WriteAllText($script:SettingsPath, $json, (New-Object System.Text.UTF8Encoding($false)))
    if ($script:UniqueCheck.Checked) { Set-Status 'Unique item builds enforced - duplicates get replaced with same-category items.' }
    else { Set-Status 'Unique enforcement off - champions may build duplicate items.' }
  }
  catch {
    Set-Status "Could not save mod-settings.json: $($_.Exception.Message)" $true
  }
}

# --- helpers ---------------------------------------------------------------
function New-Combo($displayList, $left, $width, [switch]$Grouped) {
  $cb = New-Object System.Windows.Forms.ComboBox
  $cb.DropDownStyle = 'DropDownList'
  $cb.FlatStyle = 'Flat'
  $cb.Left = $left; $cb.Top = 4; $cb.Width = $width; $cb.Height = 32
  $cb.BackColor = $cPanel2; $cb.ForeColor = $cText
  if ($Grouped) {
    $cb.DrawMode = [System.Windows.Forms.DrawMode]::OwnerDrawFixed
    $cb.ItemHeight = 24
    $cb.MaxDropDownItems = 18
    $cb.DropDownWidth = [Math]::Max($width, 240)
    $cb.Add_DrawItem($script:OnItemDraw)
  }
  foreach ($d in $displayList) { [void]$cb.Items.Add($d) }
  $cb.SelectedIndex = 0
  return $cb
}

function Set-ComboId($cb, $idToDisplay, $id) {
  if ($id -and $idToDisplay.ContainsKey($id)) { $cb.SelectedItem = $idToDisplay[$id] }
  else { $cb.SelectedIndex = 0 }
}

function Get-ComboId($cb, $displayToId) {
  $sel = [string]$cb.SelectedItem
  if ($displayToId.ContainsKey($sel)) { return $displayToId[$sel] }
  return ''
}

# Swap two of a row's item selections (used by the swap buttons between slots).
function Swap-ItemValues($row, $a, $b) {
  $items = $row.Tag.Items
  $tmp = $items[$a].SelectedItem
  $items[$a].SelectedItem = $items[$b].SelectedItem
  $items[$b].SelectedItem = $tmp
  Save-Builds
}

# Small "swap with the slot to my right" button; $index is the left slot (0 or 1).
# Text is a runtime-built glyph so this script can stay pure ASCII (no BOM needed).
function New-SwapButton($left, $index) {
  $b = New-Object System.Windows.Forms.Button
  $b.Text = [string][char]0x21C4   # left-right swap arrows
  $b.Left = $left; $b.Top = 4; $b.Width = 30; $b.Height = 32
  $b.FlatStyle = 'Flat'; $b.BackColor = $cPanel2; $b.ForeColor = $cAccent
  $b.FlatAppearance.BorderSize = 0
  $b.Tag = $index
  $b.Add_Click({ $i = [int]$this.Tag; Swap-ItemValues $this.Parent $i ($i + 1) })
  return $b
}

# Champion: pick from the list, or choose "(modded champion)" and type a raw id
# into the per-row textbox.
function Set-ChampValue($row, $id) {
  $cb = $row.Tag.Champ; $tb = $row.Tag.ChampRaw
  if (-not $id) { $cb.SelectedIndex = 0; $tb.Text = '' }
  elseif ($script:ChampToDisplay.ContainsKey($id)) { $cb.SelectedItem = $script:ChampToDisplay[$id] }
  else { $cb.SelectedItem = $MODDED_LABEL; $tb.Text = $id }
  Update-ChampMode $row
}

function Get-ChampValue($row) {
  $sel = [string]$row.Tag.Champ.SelectedItem
  if ($sel -eq $MODDED_LABEL) { return ([string]$row.Tag.ChampRaw.Text).Trim() }
  if ($script:ChampToId.ContainsKey($sel)) { return $script:ChampToId[$sel] }
  return ''
}

function Update-ChampMode($row) {
  $isModded = ([string]$row.Tag.Champ.SelectedItem -eq $MODDED_LABEL)
  $row.Tag.ChampRaw.Visible = $isModded
  $row.Height = if ($isModded) { 74 } else { 42 }
}

function New-Row {
  param([string]$Champ = '', [string[]]$Items = @('', '', ''))

  $row = New-Object System.Windows.Forms.Panel
  $row.Height = 42; $row.Width = 1048; $row.Margin = New-Object System.Windows.Forms.Padding(0, 0, 0, 8)
  $row.BackColor = $cPanel

  $cbChamp = New-Combo $script:ChampDisplay $COL.Champ[0] $COL.Champ[1]
  $cb1 = New-Combo $script:ItemDisplay $COL.I1[0] $COL.I1[1] -Grouped:$script:GroupItems
  $cb2 = New-Combo $script:ItemDisplay $COL.I2[0] $COL.I2[1] -Grouped:$script:GroupItems
  $cb3 = New-Combo $script:ItemDisplay $COL.I3[0] $COL.I3[1] -Grouped:$script:GroupItems

  # drag handle to reorder this whole champion line up/down. Text is a runtime
  # glyph so the script stays pure ASCII (no BOM needed).
  $grip = New-Object System.Windows.Forms.Label
  $grip.Text = [string][char]0x2261   # three-bar drag handle
  $grip.Left = $COL.Grip[0]; $grip.Top = 4; $grip.Width = $COL.Grip[1]; $grip.Height = 32
  $grip.TextAlign = 'MiddleCenter'; $grip.ForeColor = $cMuted
  $grip.Font = New-Object System.Drawing.Font('Segoe UI', 15)
  $grip.Cursor = [System.Windows.Forms.Cursors]::SizeAll
  # DoDragDrop blocks until the mouse is released; the panel's DragOver slots the
  # row into place live while it runs. We highlight here and persist on release.
  $grip.Add_MouseDown({
      $row = $this.Parent
      $row.BackColor = $cDragRow
      [void]$this.DoDragDrop($row, [System.Windows.Forms.DragDropEffects]::Move)
      Save-Builds   # writes the new order; also recolors rows by state
    })

  # swap buttons reorder the 3 items (each swaps with its right-hand neighbour)
  $swap1 = New-SwapButton $COL.Swap1[0] 0
  $swap2 = New-SwapButton $COL.Swap2[0] 1

  # raw-id box shown only when "(modded champion)" is selected
  $tbRaw = New-Object System.Windows.Forms.TextBox
  $tbRaw.Left = $COL.Champ[0]; $tbRaw.Top = 40; $tbRaw.Width = $COL.Champ[1]
  $tbRaw.BackColor = $cPanel2; $tbRaw.ForeColor = $cText; $tbRaw.BorderStyle = 'FixedSingle'
  $tbRaw.Visible = $false
  $tbRaw.Add_HandleCreated({ [void][Native.Edit]::SendMessage($this.Handle, 0x1501, 1, 'modded champion ID') })

  $del = New-Object System.Windows.Forms.Button
  $del.Text = 'X'; $del.Left = $COL.Del[0]; $del.Top = 4; $del.Width = $COL.Del[1]; $del.Height = 32
  $del.FlatStyle = 'Flat'; $del.BackColor = $cPanel; $del.ForeColor = $cMuted
  $del.Tag = $row

  $row.Tag = @{ Champ = $cbChamp; ChampRaw = $tbRaw; Items = @($cb1, $cb2, $cb3) }
  $row.Controls.AddRange(@($grip, $cbChamp, $tbRaw, $cb1, $swap1, $cb2, $swap2, $cb3, $del))

  Set-ChampValue $row $Champ
  Set-ComboId $cb1 $script:ItemToDisplay $Items[0]
  Set-ComboId $cb2 $script:ItemToDisplay $Items[1]
  Set-ComboId $cb3 $script:ItemToDisplay $Items[2]

  # attach AFTER setting values so loading doesn't autosave
  $cbChamp.Add_SelectedIndexChanged({ Update-ChampMode $this.Parent; Save-Builds })
  foreach ($cb in @($cb1, $cb2, $cb3)) {
    $cb.Tag = $cb.SelectedIndex   # last real (non-header) pick, for the guard below
    $cb.Add_SelectedIndexChanged({
        $i = $this.SelectedIndex
        if ($i -ge 0 -and $script:ItemHeaderIndices.Contains($i)) {
          # Category headers aren't selectable: skip to the next real item in the
          # direction of travel, snapping back if we run off either end.
          $prev = [int]$this.Tag
          $dir = if ($i -lt $prev) { -1 } else { 1 }
          $j = $i + $dir
          while ($j -ge 0 -and $j -lt $this.Items.Count -and $script:ItemHeaderIndices.Contains($j)) { $j += $dir }
          if ($j -lt 0 -or $j -ge $this.Items.Count) { $j = $prev }
          $this.SelectedIndex = $j
          return
        }
        $this.Tag = $i
        Save-Builds
      })
  }
  $tbRaw.Add_Leave({ Save-Builds })
  $del.Add_Click({
      $script:RowsPanel.Controls.Remove($this.Tag)
      Save-Builds
    })

  return $row
}

# '' empty (ignored) | 'ok' complete | 'bad' partial/invalid.
# A row is complete once it has a champion and at least one pinned item; the other
# slots default to (AI picks) and are filled by the game AI. A row with nothing
# pinned (every slot on (AI picks)) is a no-op - identical to not listing the
# champion - so it is ignored rather than flagged, even with a champion selected.
function Get-RowState($row) {
  $champ = Get-ChampValue $row
  $pinned = @($row.Tag.Items | ForEach-Object { Get-ComboId $_ $script:ItemToId } |
    Where-Object { $_ -and $_ -ne $AI_SENTINEL })
  if ($pinned.Count -eq 0) { return '' }
  if ($champ) { return 'ok' }
  return 'bad'   # items pinned but no champion selected
}

function Get-Builds {
  $out = [ordered]@{}
  foreach ($row in $script:RowsPanel.Controls) {
    if ((Get-RowState $row) -ne 'ok') { continue }
    $champ = Get-ChampValue $row
    # All slots in order: a pinned item id, or the AI sentinel for (AI picks) slots.
    # Positions are preserved (no filtering) so blanks become JSON null the mod fills.
    $slots = @($row.Tag.Items | ForEach-Object {
        $id = Get-ComboId $_ $script:ItemToId
        if ($id) { $id } else { $AI_SENTINEL }
      })
    $out[$champ] = $slots   # later duplicate rows overwrite -> last wins
  }
  return $out
}

function ConvertTo-ItemBuildsJson($ordered) {
  if ($ordered.Count -eq 0) { return "{}`n" }
  $sb = New-Object System.Text.StringBuilder
  [void]$sb.Append("{`n")
  $keys = @($ordered.Keys)
  for ($i = 0; $i -lt $keys.Count; $i++) {
    $k = $keys[$i]; $items = $ordered[$k]
    [void]$sb.Append("  `"$k`": [`n")
    for ($j = 0; $j -lt $items.Count; $j++) {
      $tail = if ($j -lt $items.Count - 1) { ',' } else { '' }
      $val = if ($items[$j] -eq $AI_SENTINEL) { 'null' } else { "`"$($items[$j])`"" }
      [void]$sb.Append("    $val$tail`n")
    }
    $tail = if ($i -lt $keys.Count - 1) { ',' } else { '' }
    [void]$sb.Append("  ]$tail`n")
  }
  [void]$sb.Append("}`n")
  return $sb.ToString()
}

function Set-Status($msg, [bool]$warn = $false) {
  $script:StatusLabel.Text = $msg
  $script:StatusLabel.ForeColor = if ($warn) { $cDanger } else { $cMuted }
}

function Test-Rows {
  # Colors each row by state and returns the incomplete count + duplicate champions.
  $bad = 0; $counts = @{}
  foreach ($row in $script:RowsPanel.Controls) {
    $state = Get-RowState $row
    if ($state -eq 'bad') { $bad++; $row.BackColor = $cBadRow } else { $row.BackColor = $cPanel }
    $champ = Get-ChampValue $row
    # Only rows that actually contribute a build count toward duplicates; an
    # ignored all-AI row ('') does not conflict with a real build for that champ.
    if ($champ -and $state -ne '') { $counts[$champ] = 1 + ([int]$counts[$champ]) }
  }
  return @{
    Bad   = $bad
    Dupes = @($counts.Keys | Where-Object { $counts[$_] -gt 1 } | ForEach-Object { Format-Name $_ })
  }
}

function Get-ValidationMessages($v) {
  $msgs = @()
  if ($v.Bad) { $msgs += "$($v.Bad) build(s) incomplete - select a champion (not saved)." }
  if ($v.Dupes.Count) { $msgs += "Duplicate: $($v.Dupes -join ', ') - last row wins." }
  return $msgs
}

function Update-Validation {
  $v = Test-Rows
  $msgs = Get-ValidationMessages $v
  if ($msgs.Count) { Set-Status ($msgs -join '   ') $true }
  else { Set-Status "$($script:RowsPanel.Controls.Count) row(s). Target: $script:TargetPath" }
}

# Hides rows whose champion doesn't contain the search text. Purely visual:
# hidden rows are still saved (Get-Builds walks the control collection, not
# visibility). Matches against both the champion id and its display name, so
# "magic" or "Magic Knight" both find magic_knight; modded raw ids match too.
function Update-Filter {
  $q = ([string]$script:SearchBox.Text).Trim().ToLower()
  $script:RowsPanel.SuspendLayout()
  foreach ($row in $script:RowsPanel.Controls) {
    if (-not $q) { $row.Visible = $true; continue }
    $id = Get-ChampValue $row
    $display = if ($id -and $script:ChampToDisplay.ContainsKey($id)) { $script:ChampToDisplay[$id] } else { $id }
    $row.Visible = ("$id $display").ToLower().Contains($q)
  }
  $script:RowsPanel.ResumeLayout()
  if ($q) {
    $total = $script:RowsPanel.Controls.Count
    $shown = @($script:RowsPanel.Controls | Where-Object { $_.Visible }).Count
    Set-Status "Filter: showing $shown of $total build(s)."
  }
  else { Update-Validation }
}

function Import-Builds($path) {
  $script:RowsPanel.SuspendLayout()
  $script:RowsPanel.Controls.Clear()
  if (Test-Path -LiteralPath $path) {
    try {
      $raw = Get-Content -Raw -LiteralPath $path -Encoding UTF8
      $data = if ([string]::IsNullOrWhiteSpace($raw)) { $null } else { $raw | ConvertFrom-Json }
    }
    catch {
      [System.Windows.Forms.MessageBox]::Show("Could not parse $([IO.Path]::GetFileName($path)):`n$($_.Exception.Message)",
        'Parse error', 'OK', 'Error') | Out-Null
      $data = $null
    }
    if ($data) {
      foreach ($prop in $data.PSObject.Properties) {
        $vals = @($prop.Value)
        # A present JSON null or missing slot -> (AI picks); a real key -> that key.
        $slots = @(0, 1, 2 | ForEach-Object {
            if ($_ -lt $vals.Count -and $null -eq $vals[$_]) { $AI_SENTINEL } else { "$($vals[$_])" }
          })
        $row = New-Row $prop.Name $slots
        [void]$script:RowsPanel.Controls.Add($row)
      }
    }
  }
  else {
    [System.Windows.Forms.MessageBox]::Show("No item-builds.json found in:`n$([IO.Path]::GetDirectoryName($path))`n`nIt will be created when you Save.",
      'No file yet', 'OK', 'Information') | Out-Null
  }
  if ($script:RowsPanel.Controls.Count -eq 0) { [void]$script:RowsPanel.Controls.Add((New-Row)) }
  $script:RowsPanel.ResumeLayout()
  Update-Validation
}

function Save-Builds {
  # Autosave: validate, write the complete builds, and report inline (no popups,
  # since this fires on every change). Incomplete builds show an error instead.
  $v = Test-Rows
  $builds = Get-Builds
  try {
    [System.IO.File]::WriteAllText($script:TargetPath, (ConvertTo-ItemBuildsJson $builds), (New-Object System.Text.UTF8Encoding($false)))
  }
  catch {
    Set-Status "Autosave failed: $($_.Exception.Message)" $true
    return
  }
  $msgs = Get-ValidationMessages $v
  if ($msgs.Count) { Set-Status (($msgs -join '   ') + "   ($($builds.Count) saved)") $true }
  else { Set-Status "Autosaved $($builds.Count) champion build(s) - $(Get-Date -Format 'HH:mm:ss')" }
}

# --- build UI --------------------------------------------------------------
$form = New-Object System.Windows.Forms.Form
$form.Text = 'Item Build Editor'
$form.StartPosition = 'CenterScreen'
$form.ClientSize = New-Object System.Drawing.Size(1105, 825)
$form.BackColor = $cBg; $form.ForeColor = $cText
$form.Font = New-Object System.Drawing.Font('Segoe UI', 11.25)

# toolbar
$toolbar = New-Object System.Windows.Forms.Panel
$toolbar.Dock = 'Top'; $toolbar.Height = 60; $toolbar.BackColor = $cPanel2

function New-ToolButton($text, $left, $primary) {
  $b = New-Object System.Windows.Forms.Button
  $b.Text = $text; $b.Left = $left; $b.Top = 11; $b.Width = 138; $b.Height = 38
  $b.FlatStyle = 'Flat'
  if ($primary) { $b.BackColor = $cAccent; $b.ForeColor = $cPanel2 }
  else { $b.BackColor = $cPanel; $b.ForeColor = $cText }
  return $b
}

$btnSave = New-ToolButton 'Save Item Builds' 15 $true
$btnSave.Width = 175
$btnAdd = New-ToolButton '+ Add Champion' 205 $false
$btnAdd.Width = 162

$script:StatusLabel = New-Object System.Windows.Forms.Label
$script:StatusLabel.AutoSize = $false
$script:StatusLabel.Left = 382; $script:StatusLabel.Top = 11; $script:StatusLabel.Width = 700; $script:StatusLabel.Height = 38
$script:StatusLabel.TextAlign = 'MiddleLeft'; $script:StatusLabel.ForeColor = $cMuted
$script:StatusLabel.Anchor = 'Top,Left,Right'

$toolbar.Controls.AddRange(@($btnSave, $btnAdd, $script:StatusLabel))

# column header
$header = New-Object System.Windows.Forms.Panel
$header.Dock = 'Top'; $header.Height = 32; $header.BackColor = $cBg
function New-HeaderLabel($text, $left, $width) {
  $l = New-Object System.Windows.Forms.Label
  $l.Text = $text.ToUpper(); $l.Left = $left + 10; $l.Top = 8; $l.Width = $width; $l.Height = 22
  $l.ForeColor = $cMuted; $l.Font = New-Object System.Drawing.Font('Segoe UI', 10)
  return $l
}
$header.Controls.AddRange(@(
    (New-HeaderLabel 'Champion' $COL.Champ[0] $COL.Champ[1]),
    (New-HeaderLabel 'Item 1' $COL.I1[0] $COL.I1[1]),
    (New-HeaderLabel 'Item 2' $COL.I2[0] $COL.I2[1]),
    (New-HeaderLabel 'Item 3' $COL.I3[0] $COL.I3[1])
  ))

# search / filter bar
$searchBar = New-Object System.Windows.Forms.Panel
$searchBar.Dock = 'Top'; $searchBar.Height = 46; $searchBar.BackColor = $cPanel2

$searchLabel = New-Object System.Windows.Forms.Label
$searchLabel.Text = 'Filter by champion:'; $searchLabel.Left = 15; $searchLabel.Top = 14
$searchLabel.Width = 145; $searchLabel.Height = 22
$searchLabel.ForeColor = $cMuted; $searchLabel.Font = New-Object System.Drawing.Font('Segoe UI', 10)

$script:SearchBox = New-Object System.Windows.Forms.TextBox
$script:SearchBox.Left = 165; $script:SearchBox.Top = 9; $script:SearchBox.Width = 320; $script:SearchBox.Height = 28
$script:SearchBox.BackColor = $cPanel2; $script:SearchBox.ForeColor = $cText; $script:SearchBox.BorderStyle = 'FixedSingle'
$script:SearchBox.Add_HandleCreated({ [void][Native.Edit]::SendMessage($this.Handle, 0x1501, 1, 'type a champion name to filter...') })
$script:SearchBox.Add_TextChanged({ Update-Filter })

$btnClear = New-Object System.Windows.Forms.Button
$btnClear.Text = 'Clear'; $btnClear.Left = 495; $btnClear.Top = 8; $btnClear.Width = 70; $btnClear.Height = 30
$btnClear.FlatStyle = 'Flat'; $btnClear.BackColor = $cPanel; $btnClear.ForeColor = $cText
$btnClear.Add_Click({ $script:SearchBox.Text = '' })

$script:UniqueCheck = New-Object System.Windows.Forms.CheckBox
# Label and color follow the state: accent-green "Enforcing unique items" while
# on, normal-text "Enforce unique items" while off.
$script:UniqueCheck.Text = if ($script:UniqueEnabled) { 'Enforcing unique items' } else { 'Enforce unique items' }
$script:UniqueCheck.Left = 600; $script:UniqueCheck.Top = 6; $script:UniqueCheck.Width = 470; $script:UniqueCheck.Height = 34
$script:UniqueCheck.Font = New-Object System.Drawing.Font('Segoe UI', 13)
$script:UniqueCheck.ForeColor = if ($script:UniqueEnabled) { $cAccent } else { $cText }
$script:UniqueCheck.Checked = $script:UniqueEnabled
$script:UniqueCheck.Add_CheckedChanged({
    $this.Text = if ($this.Checked) { 'Enforcing unique items' } else { 'Enforce unique items' }
    $this.ForeColor = if ($this.Checked) { $cAccent } else { $cText }
    Save-Settings
  })

$searchBar.Controls.AddRange(@($searchLabel, $script:SearchBox, $btnClear, $script:UniqueCheck))

# rows (scrollable)
$script:RowsPanel = New-Object System.Windows.Forms.FlowLayoutPanel
$script:RowsPanel.Dock = 'Fill'
$script:RowsPanel.FlowDirection = 'TopDown'
$script:RowsPanel.WrapContents = $false
$script:RowsPanel.AutoScroll = $true
$script:RowsPanel.BackColor = $cBg
$script:RowsPanel.Padding = New-Object System.Windows.Forms.Padding(15, 10, 15, 15)

# drag-to-reorder rows, live: as the grip is dragged, DragOver keeps slotting the
# row into its position under the cursor so the list updates in realtime. The
# grip's MouseDown handler persists the final order on release.
$script:RowsPanel.AllowDrop = $true
$script:RowsPanel.Add_DragOver({
    param($sender, $e)
    if (-not $e.Data.GetDataPresent([System.Windows.Forms.Panel])) {
      $e.Effect = [System.Windows.Forms.DragDropEffects]::None; return
    }
    $e.Effect = [System.Windows.Forms.DragDropEffects]::Move
    $dragged = $e.Data.GetData([System.Windows.Forms.Panel])
    $ctrls = $script:RowsPanel.Controls

    # Desired index = how many other visible rows have their midpoint above the
    # cursor. $e.Y is a screen coord, so measure each row's top in screen space
    # too (PointToScreen accounts for scrolling). Rows are laid out top-to-bottom
    # in index order, so we can stop at the first row that sits below the cursor.
    $k = 0
    foreach ($c in $ctrls) {
      if ($c -eq $dragged -or -not $c.Visible) { continue }
      $top = $c.PointToScreen([System.Drawing.Point]::Empty).Y
      if (($top + ($c.Height / 2)) -lt $e.Y) { $k++ } else { break }
    }
    if ($k -ne $ctrls.GetChildIndex($dragged)) {
      $script:RowsPanel.SuspendLayout()
      $ctrls.SetChildIndex($dragged, $k)
      $script:RowsPanel.ResumeLayout()
    }
  })
$script:RowsPanel.Add_DragDrop({
    param($sender, $e)
    if ($e.Data.GetDataPresent([System.Windows.Forms.Panel])) { $e.Effect = [System.Windows.Forms.DragDropEffects]::Move }
  })

# add in this order so docked Top bars stack above the Fill panel
# (later-added Top docks sit higher: toolbar, then searchBar, then header)
$form.Controls.Add($script:RowsPanel)
$form.Controls.Add($header)
$form.Controls.Add($searchBar)
$form.Controls.Add($toolbar)

# wire buttons
$btnAdd.Add_Click({
    # clear any active filter so the new (championless) row isn't hidden
    $script:SearchBox.Text = ''
    $row = New-Row
    [void]$script:RowsPanel.Controls.Add($row)
    $script:RowsPanel.ScrollControlIntoView($row)
    Save-Builds
  })
$btnSave.Add_Click({ Save-Builds })

# initial load from the script folder
Import-Builds $script:TargetPath


[void]$form.ShowDialog()
[void]$form.ShowDialog()
[void]$form.ShowDialog()
[void]$form.ShowDialog()
[void]$form.ShowDialog()
[void]$form.ShowDialog()
