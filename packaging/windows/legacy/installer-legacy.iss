#define MyAppVersion "0.2.0"

[Setup]
AppName=TicTacToe Client (Legacy)
AppVersion={#MyAppVersion}
AppId=CodeAlchemy-Labs.TicTacToe.Client.Legacy
DefaultDirName={autopf}\TicTacToe Client (Legacy)
DefaultGroupName=TicTacToe Client (Legacy)
OutputDir=..\..\dist\windows\legacy
OutputBaseFilename={#OutputBaseName}
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest
InfoBeforeFile=legacy-warning.txt

[Files]
Source: "..\..\target\{#TargetArch}\release\tictacli.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\TicTacToe Client (Legacy)"; Filename: "{app}\tictacli.exe"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
