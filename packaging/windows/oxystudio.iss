; Inno Setup script for the OxyStudio Windows installer.
;
; Built in CI by .github/workflows/release.yml:
;   iscc /DAppVersion=0.1.0 /DNumericVersion=0.1.0.0 /DSourceDir=<dir with oxystudio.exe> packaging\windows\oxystudio.iss
;
; Installs per-user by default (no admin prompt); the user may choose an
; all-users install instead. Adds Start-menu (and optional desktop)
; shortcuts and an uninstaller.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef NumericVersion
  #define NumericVersion "0.1.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\x86_64-pc-windows-msvc\release"
#endif
#define RepoRoot "..\.."

[Setup]
; Unique AppId identifying OxyStudio for upgrades and uninstalls.
AppId={{8B1A24C9-5F32-498A-8BC4-E52F81B855A3}
AppName=OxyStudio
AppVersion={#AppVersion}
AppVerName=OxyStudio {#AppVersion}
AppPublisher=OxyStudio contributors
AppPublisherURL=https://github.com/crisprintsstuff/Oxy-Launcher
AppSupportURL=https://github.com/crisprintsstuff/Oxy-Launcher/issues
AppUpdatesURL=https://github.com/crisprintsstuff/Oxy-Launcher/releases
VersionInfoVersion={#NumericVersion}
VersionInfoProductName=OxyStudio
VersionInfoDescription=OxyStudio launcher and hub
DefaultDirName={autopf}\OxyStudio
DefaultGroupName=OxyStudio
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
UninstallDisplayIcon={app}\oxystudio.exe
UninstallDisplayName=OxyStudio {#AppVersion}
SetupIconFile={#RepoRoot}\packaging\icons\oxystudio.ico
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#RepoRoot}\dist
OutputBaseFilename=OxyStudio-{#AppVersion}-windows-x86_64-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\oxystudio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\packaging\icons\oxystudio.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\OxyStudio"; Filename: "{app}\oxystudio.exe"; IconFilename: "{app}\oxystudio.ico"
Name: "{autodesktop}\OxyStudio"; Filename: "{app}\oxystudio.exe"; IconFilename: "{app}\oxystudio.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\oxystudio.exe"; Description: "{cm:LaunchProgram,OxyStudio}"; Flags: nowait postinstall skipifsilent
