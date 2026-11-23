param()
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
$root=Join-Path $project '.local/vision'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$revision='52d6c8ffea26cc873ac5ad116f8631268d7eb503'
$files=@(
 @{name='llama-b6907-bin-win-cpu-x64.zip';url='https://github.com/ggml-org/llama.cpp/releases/download/b6907/llama-b6907-bin-win-cpu-x64.zip';sha256='92d5b71fda1035ce5f65ed823568e0905bfe6fff4f7d142f550bceb06ac09dfd'},
 @{name='Qwen3VL-2B-Instruct-Q8_0.gguf';url="https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/$revision/Qwen3VL-2B-Instruct-Q8_0.gguf";sha256='1e8db19207c8ce0733ddd78c2eff8a9e22c27c82f4443df94c25792ed8fe04f2'},
 @{name='mmproj-Qwen3VL-2B-Instruct-F16.gguf';url="https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/$revision/mmproj-Qwen3VL-2B-Instruct-F16.gguf";sha256='c3d5afbef5287953acd57b4043d2269456e5761a4eaccb3b71b062996970aea5'}
)
foreach($file in $files){
 $destination=Join-Path $root $file.name
 if(!(Test-Path -LiteralPath $destination) -or (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256){
  Write-Output ('Downloading '+$file.name)
  & curl.exe --fail --location --retry 3 --silent --show-error --output ($destination+'.part') ($file.url+'?download=true')
  if($LASTEXITCODE -ne 0){throw ('Download failed: '+$file.name)}
  if((Get-FileHash -LiteralPath ($destination+'.part') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256){throw ('Checksum mismatch: '+$file.name)}
  Move-Item -LiteralPath ($destination+'.part') -Destination $destination -Force
 }
 Write-Output ('Verified '+$file.name)
}
Expand-Archive -LiteralPath (Join-Path $root $files[0].name) -DestinationPath (Join-Path $root 'llama-b6907') -Force
$server=Get-ChildItem (Join-Path $root 'llama-b6907') -Filter llama-server.exe -Recurse | Select-Object -First 1
if(!$server){throw 'llama-server executable missing'}
$record=@{modelId='Qwen/Qwen3-VL-2B-Instruct-GGUF';revision=$revision;license='Apache-2.0';runtimeVersion='b6907';runtimeLicense='MIT';server=$server.FullName;serverSha256=(Get-FileHash $server.FullName -Algorithm SHA256).Hash.ToLowerInvariant();model=(Join-Path $root $files[1].name);modelSha256=$files[1].sha256;projector=(Join-Path $root $files[2].name);projectorSha256=$files[2].sha256;files=$files;runtimeFiles=@(Get-ChildItem $server.DirectoryName -File | Where-Object { $_.Extension -in '.exe','.dll' } | ForEach-Object { @{path=$_.FullName;sha256=(Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()} })}
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'source.json') -Encoding utf8
Invoke-WebRequest 'https://raw.githubusercontent.com/ggml-org/llama.cpp/b6907/LICENSE' -OutFile (Join-Path $root 'LLAMA-LICENSE.txt')
Invoke-WebRequest 'https://www.apache.org/licenses/LICENSE-2.0.txt' -OutFile (Join-Path $root 'QWEN-LICENSE.txt')
Write-Output 'Local vision runtime and model verified. No photos processed.'
