param([Parameter(Mandatory)][string]$ImagePath, [string]$ExecutablePath)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if (!$ExecutablePath) {
    $candidates = @('target/release/shoot-to-post-scorer.exe','target/x86_64-pc-windows-gnu/release/shoot-to-post-scorer.exe')
    $ExecutablePath = $candidates | ForEach-Object { Join-Path $project $_ } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (!$ExecutablePath) { throw 'Build the scoring executable first.' }
$model = Join-Path $project '.local/hyperiqa.onnx'
$record = Get-Content -LiteralPath (Join-Path $project '.local/hyperiqa.source.json') -Raw | ConvertFrom-Json
$runtime = Join-Path $project '.local/export-packages/onnxruntime/capi/onnxruntime.dll'
& $ExecutablePath --model $model --model-sha256 $record.onnxSha256 --runtime $runtime --image $ImagePath
if ($LASTEXITCODE -ne 0) { throw 'Photo scoring failed; no result was saved to the archive.' }
