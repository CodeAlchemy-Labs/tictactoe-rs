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

[Files]
Source: "{#SourceBinary}"; DestDir: "{app}"; DestName: "tictacli.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\TicTacToe Client (Legacy)"; Filename: "{app}\tictacli.exe"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
