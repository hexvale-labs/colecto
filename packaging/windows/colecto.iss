#ifndef AppVersion
  #define AppVersion "0.0.1"
#endif
#ifndef StageDir
  #define StageDir "..\..\stage"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif
#ifndef OutputBaseName
  #define OutputBaseName "colecto-windows-x64"
#endif

[Setup]
AppId={{8D6B4F7A-9C2E-4E1A-A3B5-7F0D2E9C1A4B}
AppName=Colecto
AppVersion={#AppVersion}
AppPublisher=Kogical
AppPublisherURL=https://github.com/kogical/colecto
AppSupportURL=https://github.com/kogical/colecto/issues
DefaultDirName={autopf}\Colecto
DefaultGroupName=Colecto
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseName}
SetupIconFile=..\..\app\windows\runner\resources\app_icon.ico
UninstallDisplayIcon={app}\colecto.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Colecto"; Filename: "{app}\colecto.exe"
Name: "{autodesktop}\Colecto"; Filename: "{app}\colecto.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\colecto.exe"; Description: "{cm:LaunchProgram,Colecto}"; Flags: nowait postinstall skipifsilent
