param([int]$Port=8090,[string]$DatabasePath)
$ErrorActionPreference='Stop'
if (!$DatabasePath) { throw 'Укажите -DatabasePath с отдельной тестовой базой. Основную базу приложения не используйте во время миграции.' }
$jdk=if($env:JAVA_HOME){$env:JAVA_HOME}else{(Get-ChildItem (Join-Path $PSScriptRoot '../tools/java/runtime') -Directory | Select-Object -First 1).FullName}
$socketDirectory=Join-Path $PSScriptRoot 'build/sockets'
New-Item -ItemType Directory -Path $socketDirectory -Force | Out-Null
& (Join-Path $jdk 'bin/java.exe') "-Djdk.net.unixdomain.tmpdir=$socketDirectory" --enable-native-access=ALL-UNNAMED -cp ((Join-Path $PSScriptRoot 'build/nova-server.jar')+';'+(Join-Path $PSScriptRoot 'lib/*')) nova.Main --database $DatabasePath --port $Port --web (Join-Path $PSScriptRoot '../web/build')
