$env:RELEASE=1
$env:APKSIGNER="D:\Programs\Android\Sdk\build-tools\36.0.0\apksigner.bat"
$env:PATH = "D:\Programs\Android\Sdk\platform-tools;$env:PATH"
$env:SevenZip = 'C:\Program Files\7-Zip\7z.exe'
$Key="D:\Programs\Dev\keystore\release-key.keystore"
$Base_apk="D:\UserFiles\Downloads\EdgeDownloads\jp.co.cygames.umamusume-600-121953787-1763098263.apk"
$Config_apk="D:\UserFiles\Downloads\EdgeDownloads\jp.co.cygames.umamusume-600-config.arm64_v8a-76646482-1763098263.apk"
powershell -File ./tools/android/powershell/dev_nr.ps1 $Key $Base_apk $Config_apk