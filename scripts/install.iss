; DSHPlusPlus installer (Inno Setup 6).
; Usage: ISCC.exe /dSourceDir=<stage-dir> scripts\install.iss
; The stage dir is the assembled portable directory (DSHPlusPlus-<version>-windows-x64).
; User data is redirected via the DSHPLUSPLUS_DATA_ROOT environment variable to
; %LOCALAPPDATA%\DSHPlusPlus (Program Files is not writable; data must live outside).

#ifndef SourceDir
  #define SourceDir "..\release"
#endif

#define MyAppName "DSHPlusPlus"
#define MyAppVersion "0.1.0-dev.1"
#define MyAppPublisher "DSH++"
#define MyAppExeName "DSHPlusPlus.exe"

[Setup]
AppId={{8F3C5E2A-1B4D-4E6F-9A2B-DSHPLUSPLUS01}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\release
OutputBaseFilename=Setup-DSHPlusPlus-{#MyAppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "chinesesimp"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; All contents of the stage dir (runtime data .portable, backups and staged
; updates are excluded).
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs; Excludes: ".portable,*.bak-*,*.update.exe"

[Registry]
; Per-user data root outside Program Files.
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "DSHPLUSPLUS_DATA_ROOT"; ValueData: "{localappdata}\DSHPlusPlus"; Flags: preservestringtype

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Code]
{ Uninstall keeps user data: DSHPLUSPLUS_DATA_ROOT is left untouched. }
