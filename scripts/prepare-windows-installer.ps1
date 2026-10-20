[CmdletBinding()]
param([Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
$Destination = [IO.Path]::GetFullPath($Destination)
$archive = Join-Path ([IO.Path]::GetTempPath()) ('riina-inno-' + [guid]::NewGuid() + '.exe')
Invoke-WebRequest 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' -OutFile $archive
if ((Get-FileHash -LiteralPath $archive).Hash.ToLower() -ne '9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732') {
    throw 'Pinned Inno Setup download checksum differs'
}
$signature = Get-AuthenticodeSignature -LiteralPath $archive
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Pyrsys B.V.') {
    throw 'Inno Setup publisher signature is not valid'
}
$process = Start-Process -FilePath $archive -WindowStyle Hidden -PassThru -ArgumentList @(
    '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER','/NOICONS',('/DIR="' + $Destination + '"'))
if (!$process.WaitForExit(90000)) { Stop-Process -Id $process.Id; throw 'Compiler setup timed out' }
if ($process.ExitCode -ne 0) { throw 'Compiler setup failed' }
Write-Output (Join-Path $Destination 'ISCC.exe')
