; Inno Setup 6 script for Restaurant Operating System (Windows 10+ x64).
; Placeholders {{SOURCE_DIR}}, {{OUTPUT_DIR}}, {{OUTPUT_BASENAME}} are replaced
; by scripts/build-windows-release.ps1 before compilation.

#define MyAppName "Restaurant Operating System"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Gotigin"
#define MyAppURL "https://gotigin.com"
#define MyAppExeName "ros.exe"

[Setup]
AppId={{A7C3E2B1-9F44-4D2A-8C11-ROSCOMMUNITY01}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={autopf}\Gotigin\RestaurantOperatingSystem
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir={{OUTPUT_DIR}}
OutputBaseFilename={{OUTPUT_BASENAME}}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{{SOURCE_DIR}}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
