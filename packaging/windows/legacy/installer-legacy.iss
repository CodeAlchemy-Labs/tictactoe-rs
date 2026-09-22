#ifndef TargetArch
  #error TargetArch is required. Pass /DTargetArch=x86_64-pc-windows-gnu or i686-pc-windows-gnu
#endif

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#ifndef SourceBinary
  #error SourceBinary define is required. Pass /DSourceBinary=<absolute path to signed .exe>
#endif

#ifndef OutputDir
  #error OutputDir define is required. Pass /DOutputDir=<absolute path to output directory>
#endif

#ifndef IconFile
  #error IconFile define is required. Pass /DIconFile=<absolute path to the .ico file>
#endif

[Setup]
AppName=TicTacToe Client (Legacy)
AppVersion={#AppVersion}
AppId=CodeAlchemy-Labs.TicTacToe.Client.Legacy
DefaultDirName={autopf}\TicTacToe Client (Legacy)
DefaultGroupName=TicTacToe Client (Legacy)
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseName}
SetupIconFile={#IconFile}
UninstallDisplayIcon={app}\tictacli.exe
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest
InfoBeforeFile=legacy-warning.txt
#if TargetArch == "x86_64-pc-windows-gnu"
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#else
ArchitecturesAllowed=i686compatible
#endif

[Files]
Source: "{#SourceBinary}"; DestDir: "{app}"; DestName: "tictacli.exe"; Flags: ignoreversion
Source: "{#IconFile}"; DestDir: "{app}"; DestName: "tictacli.ico"; Flags: ignoreversion

[Icons]
Name: "{group}\TicTacToe Client (Legacy)"; Filename: "{app}\tictacli.exe"; IconFilename: "{app}\tictacli.ico"
Name: "{autodesktop}\TicTacToe Client (Legacy)"; Filename: "{app}\tictacli.exe"; IconFilename: "{app}\tictacli.ico"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
