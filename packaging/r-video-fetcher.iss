#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#define AppName "r-video-fetcher"
#define AppExeName "r-video-fetcher.exe"

[Setup]
AppId={{B018CF94-2F88-4D9A-9406-36A3D45A821C}
AppName={#AppName}
AppVersion={#AppVersion}
DefaultDirName={autopf}\r-video-fetcher
DefaultGroupName={#AppName}
SetupIconFile=..\src\resources\icon.ico
UninstallDisplayIcon={app}\{#AppExeName}
OutputDir=..\dist
OutputBaseFilename=r-video-fetcher-setup-{#AppVersion}
Compression=lzma
SolidCompression=yes
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
PrivilegesRequired=admin
CloseApplications=yes

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked
