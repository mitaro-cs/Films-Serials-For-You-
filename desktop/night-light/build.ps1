$ErrorActionPreference='Stop'
$resources=Join-Path $PSScriptRoot '../src-tauri/resources/night'
New-Item -ItemType Directory -Path $resources -Force | Out-Null
$sdk=Join-Path $PSScriptRoot '../../tools/mystic-light/sdk/Mystic_light_SDK_1.0.0.08/MysticLight_SDK_x64.dll'
if((Get-AuthenticodeSignature $sdk).Status -ne 'Valid') {throw 'MSI SDK signature is not valid'}
$lock=Get-Content (Join-Path $PSScriptRoot 'msi-sdk.lock.json') | ConvertFrom-Json
if((Get-FileHash $sdk -Algorithm SHA256).Hash.ToLowerInvariant() -ne $lock.dllSha256){throw 'MSI SDK SHA-256 mismatch'}
Copy-Item -LiteralPath $sdk -Destination $resources -Force
& "$env:WINDIR\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:exe /platform:x64 /optimize+ /r:System.Web.Extensions.dll "/out:$resources\NovaNightLight.exe" (Join-Path $PSScriptRoot 'NovaNightLight.cs')
if($LASTEXITCODE -ne 0){throw 'Night light helper build failed'}
