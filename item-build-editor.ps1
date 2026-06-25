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

# --- data (from item-build-data-reference.txt) -----------------------------
$CHAMPIONS = @(
  'android', 'archer', 'bard', 'barrier_magician', 'berserker', 'bomber',
  'boomerang_hunter', 'cavalry_knight', 'chef', 'circus_blade', 'clown', 'dancer',
  'dark_mage', 'demon', 'dokkaebi', 'druid', 'dual_blader', 'enchanter',
  'executioner', 'exorcist', 'fighter', 'gambler', 'ghost', 'guardian_spirit',
  'gunner', 'hammerer', 'hitman', 'hunter', 'ice_mage', 'illusionist',
  'inquisitor', 'jiangshi', 'knight', 'lancer', 'lightning_mage', 'magic_knight',
  'monk', 'necromancer', 'ninja', 'ogre', 'plague_doctor', 'poison_dart_hunter',
  'pole_warrior', 'priest', 'prisoner', 'pyromancer', 'pythoness', 'shadowmancer',
  'shield_bearer', 'siege_breaker', 'soldier', 'spirit_caller', 'swordman', 'taoist',
  'vampire', 'voodoo_shaman', 'werewolf', 'whip_master', 'white_mage', 'wind_mage'
)
$ITEMS = @(
  'blackfire_torch', 'blade_of_the_ruined_king', 'bloodthirster', 'collector',
  'deathblade', 'dragons_claw', 'experimental_hexplate', 'frozen_mallet',
  'guinsoos_rageblade', 'infinity_edge', 'jaksho_the_protean', 'ludens_tempest',
  'mirage_blade', 'mortal_reminder', 'nashors_tooth', 'phantom_dancer',
  'protectors_vow', 'protoplasm_harness', 'rabadons_deathcap', 'riftmaker',
  'spirit_visage', 'terminus', 'thornmail', 'unending_despair',
  'warmogs_armor'
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

# column geometry (x, width)
$COL = @{
  Champ = @(0, 275)
  I1    = @(288, 200)
  I2    = @(500, 200)
  I3    = @(712, 200)
  Del   = @(925, 35)
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
  $row.Height = 42; $row.Width = 965; $row.Margin = New-Object System.Windows.Forms.Padding(0, 0, 0, 8)
  $row.BackColor = $cPanel

  $cbChamp = New-Combo $script:ChampDisplay $COL.Champ[0] $COL.Champ[1]
  $cb1 = New-Combo $script:ItemDisplay $COL.I1[0] $COL.I1[1]
  $cb2 = New-Combo $script:ItemDisplay $COL.I2[0] $COL.I2[1]
  $cb3 = New-Combo $script:ItemDisplay $COL.I3[0] $COL.I3[1]

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
  $row.Controls.AddRange(@($cbChamp, $tbRaw, $cb1, $cb2, $cb3, $del))

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
$form.ClientSize = New-Object System.Drawing.Size(1025, 825)
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
$script:StatusLabel.Left = 382; $script:StatusLabel.Top = 11; $script:StatusLabel.Width = 628; $script:StatusLabel.Height = 38
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

# rows (scrollable)
$script:RowsPanel = New-Object System.Windows.Forms.FlowLayoutPanel
$script:RowsPanel.Dock = 'Fill'
$script:RowsPanel.FlowDirection = 'TopDown'
$script:RowsPanel.WrapContents = $false
$script:RowsPanel.AutoScroll = $true
$script:RowsPanel.BackColor = $cBg
$script:RowsPanel.Padding = New-Object System.Windows.Forms.Padding(15, 10, 15, 15)

# add in this order so docked Top bars stack above the Fill panel
$form.Controls.Add($script:RowsPanel)
$form.Controls.Add($header)
$form.Controls.Add($toolbar)

# wire buttons
$btnAdd.Add_Click({
    $row = New-Row
    [void]$script:RowsPanel.Controls.Add($row)
    $script:RowsPanel.ScrollControlIntoView($row)
    Save-Builds
  })
$btnSave.Add_Click({ Save-Builds })

# initial load from the script folder
Import-Builds $script:TargetPath

[void]$form.ShowDialog()
