<#
.SYNOPSIS
Packs a built Windows release as a Velopack installer and feeds (T1909):
Tenmoku-<version>-windows-<arch>-Setup.exe with its .sha256, and per channel
releases.windows-<arch>-<stable|preview>.json and the full .nupkg it lists.

.DESCRIPTION
It does not run cargo. PackDir is the unpacked Tenmoku-<version>-windows-<arch> folder that
scripts/package-windows.ps1 zips: its tenmoku.exe (built with TENMOKU_UPDATE_FEED, and
TENMOKU_UPDATE_PUBLIC_KEY when set, baked in) and Licenses\ are packed. vpk packs them with the
pack id Tenmoku once per channel of -ChannelSet (velopack-pack.sh's --channel-set):
  stable (the default)  windows-<arch>-stable; a preview version is refused
  preview               windows-<arch>-preview
  both                  a stable version on both channels, so the preview feed carries it too;
                        a preview version on windows-<arch>-preview only
(the release workflow passes both). A preview version never reaches the stable channel.
Setup.exe comes from the first pack, the stable one when there is one. It installs per user into
%LocalAppData%\Tenmoku with no administrator prompt, adds a Start menu shortcut and an
Add/Remove Programs entry that uninstalls it. vpk's portable zip is not made: the release
already has package-windows.ps1's zip.

Each feed is signed when TENMOKU_UPDATE_PRIVATE_KEY and TENMOKU_UPDATE_PUBLIC_KEY are both set:
-Signer (sign_update_feed.exe) gets the key on stdin and writes the feed's .sig, and its
public half must be TENMOKU_UPDATE_PUBLIC_KEY. Without a private key the feeds are left unsigned,
which the app refuses; a private key without the public one is refused. Setup.exe is not
Authenticode signed. Prints each file it leaves in Out and stops on any failure.

.PARAMETER Arch
x64 or arm64.

.PARAMETER Version
The release's version: X.Y.Z or X.Y.Z-preview.N.

.PARAMETER PackDir
The unpacked portable folder.

.PARAMETER Out
Where the release files go.

.PARAMETER Signer
sign_update_feed.exe, needed only to sign.

.PARAMETER ChannelSet
stable (the default), preview or both: the channels to pack on, as above.

.PARAMETER DryRun
Print the vpk and signer commands and run nothing.
#>
param(
    [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Arch,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$PackDir,
    [Parameter(Mandatory)][string]$Out,
    [string]$Signer = '',
    [ValidateSet('stable', 'preview', 'both')][string]$ChannelSet = 'stable',
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
function Die([string]$message) { throw "velopack-pack: $message" }

# -c: case-sensitive, so 1.2.3-Preview.1 is refused as the tag check refuses it.
if ($Version -cnotmatch '^[0-9]+(\.[0-9]+)*(-preview\.(0|[1-9][0-9]*))?$') {
    Die "version must be X.Y.Z or X.Y.Z-preview.N, got '$Version'"
}
$isPreview = $Version -clike '*-preview.*'
# ValidateSet matches case-insensitively: compare lower case. A preview never packs stable.
$kinds = @(switch ($ChannelSet.ToLowerInvariant()) {
        'stable' {
            if ($isPreview) { Die "a preview version goes on the preview channel only, not stable: pass -ChannelSet preview or both ('$Version')" }
            'stable'
        }
        'preview' { 'preview' }
        'both' { if ($isPreview) { 'preview' } else { 'stable'; 'preview' } }
    })
if (-not (Test-Path -PathType Leaf (Join-Path $PackDir 'tenmoku.exe'))) { Die "no tenmoku.exe in $PackDir" }
if (-not (Test-Path -PathType Container (Join-Path $PackDir 'Licenses'))) { Die "no Licenses\ in $PackDir" }

$private = [string]$env:TENMOKU_UPDATE_PRIVATE_KEY
$public = [string]$env:TENMOKU_UPDATE_PUBLIC_KEY
# The key reaches the signer only: out of the environment before vpk runs.
Remove-Item Env:TENMOKU_UPDATE_PRIVATE_KEY -ErrorAction SilentlyContinue
if ($public -and $public -cnotmatch '^[A-Za-z0-9+/=]+$') { Die 'TENMOKU_UPDATE_PUBLIC_KEY is not base64' }
if ($private -and -not $public) { Die 'a private key but no TENMOKU_UPDATE_PUBLIC_KEY: the app would have no key to check the feed with' }
if ($private -and -not $Signer) { Die '-Signer is required to sign the feed' }

$pin = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^velopack = \{ version = "=([^"]+)"').Matches |
    Select-Object -First 1 | ForEach-Object { $_.Groups[1].Value }
if (-not $pin) { Die "no pinned velopack version in Cargo.toml" }
$vpk = if ($env:VPK) { $env:VPK } else { (Get-Command vpk -ErrorAction SilentlyContinue).Source }
if (-not $vpk) { Die "no vpk in PATH; install it with: dotnet tool install vpk --version $pin --tool-path <dir>" }

$work = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
# vpk's arguments for channel windows-<Arch>-<kind>, packing into $work\vpk-<kind>.
function VpkArgs([string]$kind) {
    @('pack', '--packId', 'Tenmoku', '--packVersion', $Version, '--packDir', (Join-Path $work 'app'),
        '--mainExe', 'tenmoku.exe', '--channel', "windows-$Arch-$kind", '--runtime', "win-$Arch", '--packTitle', 'Tenmoku',
        '--packAuthors', 'Tenmoku', '--icon', (Join-Path $root 'crates\fastedit\assets\windows\tenmoku.ico'),
        '--shortcuts', 'StartMenuRoot', '--noPortable', '--delta', 'None', '--outputDir', (Join-Path $work "vpk-$kind"))
}
if ($DryRun) {
    foreach ($kind in $kinds) {
        Write-Output "$vpk $((VpkArgs $kind) -join ' ')"
        $feedFile = "releases.windows-$Arch-$kind.json"
        if ($private) { Write-Output "<private key from TENMOKU_UPDATE_PRIVATE_KEY> | $Signer sign $(Join-Path $Out $feedFile)" }
        else { Write-Output "velopack-pack: note: no private key, so $feedFile is left unsigned" }
    }
    exit 0
}

# Not $version: PowerShell names are case-insensitive, and that is $Version.
$vpkVersion = (& $vpk -h 2>&1 | Out-String) -replace '(?s).*Velopack CLI (\S+).*', '$1'
if ($vpkVersion -ne $pin) { Die "vpk is '$vpkVersion', not $pin as the velopack crate is pinned" }

New-Item -ItemType Directory -Force (Join-Path $work 'app'), $Out | Out-Null
try {
    Copy-Item (Join-Path $PackDir 'tenmoku.exe') (Join-Path $work 'app\tenmoku.exe')
    Copy-Item -Recurse (Join-Path $PackDir 'Licenses') (Join-Path $work 'app\Licenses')
    $name = "Tenmoku-$Version-windows-$Arch-Setup.exe"
    $setup = Join-Path (Resolve-Path $Out) $name
    $written = @($setup, "$setup.sha256")
    foreach ($kind in $kinds) {
        $channel = "windows-$Arch-$kind"
        $vpkArgs = VpkArgs $kind
        & $vpk @vpkArgs
        if ($LASTEXITCODE -ne 0) { Die "vpk pack exited $LASTEXITCODE for $channel" }

        $vpkOut = Join-Path $work "vpk-$kind"
        $feedFile = "releases.$channel.json"
        $setups = @(Get-ChildItem $vpkOut -Filter '*-Setup.exe')
        if ($setups.Count -ne 1) { Die "vpk did not write one Setup.exe: $($setups.Name -join ', ')" }
        $nupkg = "Tenmoku-$Version-$channel-full.nupkg"
        foreach ($f in $feedFile, $nupkg) {
            if (-not (Test-Path -PathType Leaf (Join-Path $vpkOut $f))) { Die "vpk wrote no $f" }
        }
        # One Setup.exe per release: the first channel's, stable when the version is on it.
        if ($kind -eq $kinds[0]) {
            Move-Item -Force $setups[0].FullName $setup
            $hash = (Get-FileHash -Algorithm SHA256 $setup).Hash.ToLowerInvariant()
            # LF and no BOM, as package-windows.ps1 writes its .sha256.
            [IO.File]::WriteAllText("$setup.sha256", "$hash  $name`n")
        }
        Copy-Item (Join-Path $vpkOut $feedFile), (Join-Path $vpkOut $nupkg) $Out
        $feed = Join-Path (Resolve-Path $Out) $feedFile
        $written += $feed, (Join-Path (Resolve-Path $Out) $nupkg)
        Remove-Item -Force -ErrorAction SilentlyContinue "$feed.sig"
        if ($private) {
            if (-not (Test-Path -PathType Leaf $Signer)) { Die "no signer at $Signer" }
            # On stdin only: an argument would show in the process list.
            $signed = ($private | & $Signer sign $feed | Out-String).Trim()
            if ($LASTEXITCODE -ne 0) { Die "signing $feedFile failed" }
            # -cne: base64 is case-sensitive, and -ne is not.
            if ($signed -cne $public) {
                Remove-Item -Force -ErrorAction SilentlyContinue "$feed.sig"
                Die "the private key's public half is not TENMOKU_UPDATE_PUBLIC_KEY; no signature written"
            }
            $written += "$feed.sig"
        }
        elseif ($public) {
            Write-Warning "no private key: $feedFile is unsigned, and this build's key refuses it"
        }
        else {
            Write-Output "no private key or public key: $feedFile is unsigned, and the app has no key to take it with"
        }
    }
    Write-Output $written
}
finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $work
}
