#define MyAppVersion "0.2.0"

[Setup]
AppName=TicTacToe Client Legacy
AppVersion={#MyAppVersion}
DefaultDirName={autopf}\TicTacToe Client
DefaultGroupName=TicTacToe Client
OutputDir=..\..\dist\windows
OutputBaseFilename=tictacli-legacy-setup
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest

[Files]
Source: "..\..\target\x86_64-pc-windows-gnu\release\tictacli.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\TicTacToe Client"; Filename: "{app}\tictacli.exe"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
