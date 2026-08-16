; DSHPlusPlus 安装器（Inno Setup 6）。
; 用法：ISCC.exe /dSourceDir=<发布目录> scripts\install.iss
; 示例：ISCC.exe /dSourceDir=release\DSHPlusPlus-0.1.0-dev.1-windows-x64 scripts\install.iss
; 设计：便携目录安装到 {app}；用户数据通过 DSHPLUSPLUS_DATA_ROOT 环境变量
; 落到 %LOCALAPPDATA%\DSHPlusPlus（Program Files 目录不可写，数据必须外置）。

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
; 便携目录全部内容（排除运行数据 .portable 与历史备份）
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs; Excludes: ".portable,*.bak-*,*.update.exe"

[Registry]
; 用户数据外置：数据根目录指向 %LOCALAPPDATA%\DSHPlusPlus（per-user）
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "DSHPLUSPLUS_DATA_ROOT"; ValueData: "{localappdata}\DSHPlusPlus"; Flags: preservestringtype

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Code]
{ 卸载时提示保留用户数据（DSHPLUSPLUS_DATA_ROOT 指向的目录不动，由用户自行清理）。 }
