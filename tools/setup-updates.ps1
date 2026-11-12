# Called by "Set up updates.bat". Puts the update key's public half and sompio.org into the release
# settings, and makes the two publishing codes once (kept next to your signing keys).
param([string]$KeyDir, [string]$Root)
$ErrorActionPreference = 'Stop'
$key = Join-Path $KeyDir 'update.key'
$codes = Join-Path $KeyDir 'publishing codes.txt'
$conf = Join-Path $Root 'app\src-tauri\tauri.release.conf.json'

$c = Get-Content -Raw $conf | ConvertFrom-Json
$c.plugins.updater.pubkey = (Get-Content -Raw "$key.pub").Trim()
$c.plugins.updater.endpoints = @('https://sompio.org/update.php')
[IO.File]::WriteAllText($conf, ($c | ConvertTo-Json -Depth 20))

if (-not (Test-Path $codes)) {
    function New-Code {
        $b = New-Object byte[] 24
        [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b)
        -join ($b | ForEach-Object { $_.ToString('x2') })
    }
    $p = New-Code
    $u = New-Code
    $k = (Get-Content -Raw $key).Trim()
    $lines = @(
        'SOMPIO PUBLISHING CODES. Keep this file private, like your signing keys.',
        '',
        '=== 1. GitHub: your sompio repository > Settings > Secrets and variables > Actions > New repository secret ===',
        '',
        'Name:  TAURI_SIGNING_PRIVATE_KEY',
        "Value: $k",
        '',
        'Name:  SOMPIO_PUBLISH_TOKEN',
        "Value: $p",
        '',
        'Name:  SOMPIO_UPDATE_TOKEN',
        "Value: $u",
        '',
        '=== 2. sompio.org: in cPanel File Manager, edit public_html/private/config.php and fill in these two lines ===',
        '',
        "    'publish_token' => '$p',",
        "    'update_token' => '$u',"
    )
    [IO.File]::WriteAllLines($codes, $lines)
}
