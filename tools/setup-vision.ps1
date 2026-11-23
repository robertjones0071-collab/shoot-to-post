param()
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
$root=Join-Path $project '.local/vision'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$revision='52d6c8ffea26cc873ac5ad116f8631268d7eb503'
$files=@(
 @{name='llama-b9000-bin-win-cpu-x64.zip';url='https://github.com/ggml-org/llama.cpp/releases/download/b9000/llama-b9000-bin-win-cpu-x64.zip';sha256='8294e287933d3212aa93a32e1ceb800722bede14d9692d405679d4ba77cf05db'},
 @{name='Qwen3VL-2B-Instruct-Q8_0.gguf';url="https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/$revision/Qwen3VL-2B-Instruct-Q8_0.gguf";sha256='1e8db19207c8ce0733ddd78c2eff8a9e22c27c82f4443df94c25792ed8fe04f2'},
 @{name='mmproj-Qwen3VL-2B-Instruct-Q8_0.gguf';url="https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/$revision/mmproj-Qwen3VL-2B-Instruct-Q8_0.gguf";sha256='f9a68fabba69c3b81e153367b2c7521030b0fa8bb0de400c9599c8e6725f9c82'}
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
Expand-Archive -LiteralPath (Join-Path $root $files[0].name) -DestinationPath (Join-Path $root 'llama-b9000') -Force
$server=Get-ChildItem (Join-Path $root 'llama-b9000') -Filter llama-server.exe -Recurse | Select-Object -First 1
if(!$server){throw 'llama-server executable missing'}
$record=@{modelId='Qwen/Qwen3-VL-2B-Instruct-GGUF';revision=$revision;license='Apache-2.0';runtimeVersion='b9000';runtimeLicense='MIT';cpuThreads=4;imageMinTokens=256;imageMaxTokens=1024;detailLayout='individual';server=$server.FullName;serverSha256=(Get-FileHash $server.FullName -Algorithm SHA256).Hash.ToLowerInvariant();model=(Join-Path $root $files[1].name);modelSha256=$files[1].sha256;projector=(Join-Path $root $files[2].name);projectorSha256=$files[2].sha256;files=$files;runtimeFiles=@(Get-ChildItem $server.DirectoryName -File | Where-Object { $_.Extension -in '.exe','.dll' } | ForEach-Object { @{path=$_.FullName;sha256=(Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()} })}
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'source.json') -Encoding utf8
Invoke-WebRequest 'https://raw.githubusercontent.com/ggml-org/llama.cpp/b9000/LICENSE' -OutFile (Join-Path $root 'LLAMA-LICENSE.txt')
Invoke-WebRequest 'https://www.apache.org/licenses/LICENSE-2.0.txt' -OutFile (Join-Path $root 'QWEN-LICENSE.txt')
Write-Output 'Local vision runtime and model verified. No photos processed.'
