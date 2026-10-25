$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$jdk = if ($env:JAVA_HOME) { $env:JAVA_HOME } else { (Get-ChildItem (Join-Path $root '../tools/java/runtime') -Directory | Select-Object -First 1).FullName }
if (!(Test-Path (Join-Path $jdk 'bin/javac.exe'))) { throw 'Нужен JDK 25. Задайте JAVA_HOME.' }
foreach ($dependency in (Get-Content (Join-Path $root 'dependencies.lock.json') | ConvertFrom-Json)) {
    $jar = Join-Path $root ('lib/'+$dependency.artifact+'-'+$dependency.version+'.jar')
    if (!(Test-Path $jar) -or (Get-FileHash $jar -Algorithm SHA256).Hash.ToLower() -ne $dependency.sha256) { throw "Нет проверенной зависимости: $jar" }
}
New-Item -ItemType Directory -Path (Join-Path $root 'build/classes'),(Join-Path $root 'build/test-classes') -Force | Out-Null
$socketDirectory=Join-Path $root 'build/sockets'
New-Item -ItemType Directory -Path $socketDirectory -Force | Out-Null
$sources = @(Get-ChildItem (Join-Path $root 'src/main/java') -Filter '*.java' -Recurse | Select-Object -ExpandProperty FullName)
& (Join-Path $jdk 'bin/javac.exe') --release 25 -encoding UTF-8 -Xlint:all -Werror -cp (Join-Path $root 'lib/*') -d (Join-Path $root 'build/classes') @sources
if ($LASTEXITCODE -ne 0) { throw 'Java compilation failed' }
Copy-Item (Join-Path $root 'src/main/resources/*') (Join-Path $root 'build/classes') -Recurse -Force
& (Join-Path $jdk 'bin/jar.exe') --create --file (Join-Path $root 'build/nova-server.jar') --main-class nova.Main -C (Join-Path $root 'build/classes') .
if ($LASTEXITCODE -ne 0) { throw 'Java packaging failed' }
$tests = @(Get-ChildItem (Join-Path $root 'src/test/java') -Filter '*.java' -Recurse | Select-Object -ExpandProperty FullName)
if ($tests.Count) {
    & (Join-Path $jdk 'bin/javac.exe') --release 25 -encoding UTF-8 -Xlint:all -Werror -cp ((Join-Path $root 'build/classes')+';'+(Join-Path $root 'lib/*')) -d (Join-Path $root 'build/test-classes') @tests
    if ($LASTEXITCODE -ne 0) { throw 'Java test compilation failed' }
    & (Join-Path $jdk 'bin/java.exe') "-Djdk.net.unixdomain.tmpdir=$socketDirectory" --enable-native-access=ALL-UNNAMED -ea -cp ((Join-Path $root 'build/classes')+';'+(Join-Path $root 'build/test-classes')+';'+(Join-Path $root 'lib/*')) nova.CoreTest
    if ($LASTEXITCODE -ne 0) { throw 'Java tests failed' }
}
