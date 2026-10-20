#ifndef PackageDir
  #error PackageDir must name the verified portable package
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif
#ifndef AppVersion
  #error AppVersion is required
#endif

[Setup]
AppId={{575C5AEC-C763-49F6-BF2A-0D34BB908A94}
AppName=Riina
AppVersion={#AppVersion}
AppPublisher=deminden
AppPublisherURL=https://github.com/deminden/riina
AppSupportURL=https://github.com/deminden/riina/issues
AppUpdatesURL=https://github.com/deminden/riina/releases
DefaultDirName={localappdata}\Programs\Riina
DefaultGroupName=Riina
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible and not arm64
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
OutputDir={#OutputDir}
OutputBaseFilename=riina-{#AppVersion}-windows-x86_64-setup
SetupIconFile=..\..\assets\branding\windows\riina.ico
UninstallDisplayIcon={app}\riina.exe
UninstallDisplayName=Riina
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile={#PackageDir}\LICENSE-GPL-3.0.txt
InfoBeforeFile=installer-info.txt
CloseApplications=yes
RestartApplications=no
SetupLogging=yes

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
; The builder enumerates the complete verified package, without wildcard extras.
#include PayloadList

[Icons]
Name: "{group}\Riina"; Filename: "{app}\riina.exe"; WorkingDir: "{app}"; AppUserModelID: "org.riina.player"
Name: "{autodesktop}\Riina"; Filename: "{app}\riina.exe"; WorkingDir: "{app}"; AppUserModelID: "org.riina.player"; Tasks: desktopicon

[Run]
Filename: "{app}\riina.exe"; Description: "Launch Riina"; Flags: nowait postinstall skipifsilent

; Only installed files are removed. Preferences, history and user media are outside
; {app}; deliberately omit recursive deletion and default-association changes.
