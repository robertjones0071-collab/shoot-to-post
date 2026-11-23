param([switch]$Gnu, [switch]$Test)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (!$cargo) {
    $candidate = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
    if (!(Test-Path -LiteralPath $candidate)) { throw 'Install Rust before building the scorer.' }
    $cargoPath = $candidate
} else { $cargoPath = $cargo.Source }
$toolchain = @()
$target = @()
$savedPath = $env:PATH
$savedFlags = $env:RUSTFLAGS
$savedEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
$savedLinker = $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER
Push-Location $project
try {
    if ($Gnu) {
        $toolchain = @('+stable-x86_64-pc-windows-gnu')
        $rustc = Join-Path (Split-Path -Parent $cargoPath) 'rustc.exe'
        $sysroot = & $rustc @toolchain --print sysroot
        if ($LASTEXITCODE -ne 0) { throw 'Install the stable Windows GNU Rust toolchain.' }
        $bin = Join-Path $sysroot 'lib/rustlib/x86_64-pc-windows-gnu/bin'
        $assembler = Join-Path $project '.local/binutils/mingw64/bin'
        if (!(Test-Path -LiteralPath (Join-Path $assembler 'as.exe'))) {
            throw 'GNU builds require a working MinGW assembler in .local/binutils/mingw64/bin. See docs/SCORER-PROOF.md.'
        }
        $env:PATH = $assembler + ';' + $env:PATH
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $bin 'rust-lld.exe'
        # Encoded flags preserve the dlltool path even when the profile contains spaces.
        $dlltool = (Join-Path $bin 'self-contained/dlltool.exe').Replace('\','/')
        $env:CARGO_ENCODED_RUSTFLAGS = @('-C','linker-flavor=ld.lld','-C','link-self-contained=yes','-C',('dlltool='+$dlltool)) -join [char]31
        $target = @('--target', 'x86_64-pc-windows-gnu')
    }
    $operation = if ($Test) { 'test' } else { 'build' }
    $cargoProfileArgs = @()
    if (!$Test) { $cargoProfileArgs = @('--release') }
    & $cargoPath @toolchain $operation @cargoProfileArgs @target --locked -j 2
    if ($LASTEXITCODE -ne 0) { throw 'Rust build or test failed.' }
} finally {
    Pop-Location
    $env:PATH = $savedPath
    $env:RUSTFLAGS = $savedFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $savedEncodedFlags
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $savedLinker
}
