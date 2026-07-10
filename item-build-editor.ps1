#requires -version 5.1
<#
  Item Build Editor (native WinForms GUI) for riot_items_tfm2.

  One row per champion: champion dropdown + 3 item dropdowns. Reads and writes
  item-builds.json in this script's own folder (so drop this .ps1 next to it).

  - Auto-loads item-builds.json from the script folder on start.
  - Autosaves to that file on every change (the Save button forces a save too).
  - Pick a champion, or choose "(modded champion)" to type a raw/modded id.
  - Every build needs a champion and all 3 items; incomplete builds show an
    error and are not written.

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
  'black_cleaver', 'blackfire_torch', 'blade_of_the_ruined_king', 'bloodletters_curse', 'bloodthirster', 'collector',
  'deathblade', 'deaths_dance', 'diamond_tipped_spear', 'dragons_claw', 'echoes_of_helia', 'experimental_hexplate', 'frozen_mallet',
  'guinsoos_rageblade', 'heartsteel', 'hextech_gunblade', 'infinity_edge', 'jaksho_the_protean',
  'liandrys_torment', 'ludens_tempest', 'mirage_blade', 'morellonomicon', 'mortal_reminder', 'nashors_tooth',
  'night_harvester', 'overlords_bloodmail', 'phantom_dancer',
  'protectors_vow', 'protoplasm_harness', 'rabadons_deathcap', 'riftmaker', 'rylais_crystal_scepter', 'shadowflame', 'spear_of_shojin',
  'spirit_visage', 'stormrazor', 'sunfire_cape', 'terminus', 'thornmail', 'unending_despair',
  'void_staff', 'warmogs_armor', 'yun_tal_wildarrows', 'zekes_herald'
)

$CHAMP_PLACEHOLDER = '(champion)'
$ITEM_PLACEHOLDER = '(none)'
$MODDED_LABEL = '(modded champion)'

function Format-Name($id) {
  if ($id -eq 'soldier') { return 'Soldier (Sniper)' }
  ($id -split '_' | ForEach-Object {
    if ($_.Length) { $_.Substring(0, 1).ToUpper() + $_.Substring(1) }
  }) -join ' '
}

# display <-> id maps
$script:ChampDisplay = @($CHAMP_PLACEHOLDER, $MODDED_LABEL) + ($CHAMPIONS | ForEach-Object { Format-Name $_ })
$script:ItemDisplay = @($ITEM_PLACEHOLDER) + ($ITEMS      | ForEach-Object { Format-Name $_ })
$script:ChampToId = @{ $CHAMP_PLACEHOLDER = '' }
$script:ChampToDisplay = @{}
foreach ($c in $CHAMPIONS) { $d = Format-Name $c; $script:ChampToId[$d] = $c; $script:ChampToDisplay[$c] = $d }
$script:ItemToId = @{ $ITEM_PLACEHOLDER = '' }
$script:ItemToDisplay = @{}
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

# --- helpers ---------------------------------------------------------------
function New-Combo($displayList, $left, $width) {
  $cb = New-Object System.Windows.Forms.ComboBox
  $cb.DropDownStyle = 'DropDownList'
  $cb.FlatStyle = 'Flat'
  $cb.Left = $left; $cb.Top = 4; $cb.Width = $width; $cb.Height = 32
  $cb.BackColor = $cPanel2; $cb.ForeColor = $cText
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
  $cb1 = New-Combo $script:ItemDisplay $COL.I1[0] $COL.I1[1]
  $cb2 = New-Combo $script:ItemDisplay $COL.I2[0] $COL.I2[1]
  $cb3 = New-Combo $script:ItemDisplay $COL.I3[0] $COL.I3[1]

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
  foreach ($cb in @($cb1, $cb2, $cb3)) { $cb.Add_SelectedIndexChanged({ Save-Builds }) }
  $tbRaw.Add_Leave({ Save-Builds })
  $del.Add_Click({
      $script:RowsPanel.Controls.Remove($this.Tag)
      Save-Builds
    })

  return $row
}

# '' empty (ignored) | 'ok' complete | 'bad' partial/invalid
function Get-RowState($row) {
  $champ = Get-ChampValue $row
  $items = @($row.Tag.Items | ForEach-Object { Get-ComboId $_ $script:ItemToId } | Where-Object { $_ })
  if (-not $champ -and $items.Count -eq 0) { return '' }
  if ($champ -and $items.Count -eq 3) { return 'ok' }
  return 'bad'
}

function Get-Builds {
  $out = [ordered]@{}
  foreach ($row in $script:RowsPanel.Controls) {
    if ((Get-RowState $row) -ne 'ok') { continue }
    $champ = Get-ChampValue $row
    $items = @($row.Tag.Items | ForEach-Object { Get-ComboId $_ $script:ItemToId } | Where-Object { $_ })
    $out[$champ] = $items   # later duplicate rows overwrite -> last wins
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
      [void]$sb.Append("    `"$($items[$j])`"$tail`n")
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
    if ((Get-RowState $row) -eq 'bad') { $bad++; $row.BackColor = $cBadRow } else { $row.BackColor = $cPanel }
    $champ = Get-ChampValue $row
    if ($champ) { $counts[$champ] = 1 + ([int]$counts[$champ]) }
  }
  return @{
    Bad   = $bad
    Dupes = @($counts.Keys | Where-Object { $counts[$_] -gt 1 } | ForEach-Object { Format-Name $_ })
  }
}

function Get-ValidationMessages($v) {
  $msgs = @()
  if ($v.Bad) { $msgs += "$($v.Bad) build(s) incomplete - select a champion and all 3 items (not saved)." }
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
        $items = @($prop.Value)
        $row = New-Row $prop.Name @("$($items[0])", "$($items[1])", "$($items[2])")
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

$searchBar.Controls.AddRange(@($searchLabel, $script:SearchBox, $btnClear))

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
