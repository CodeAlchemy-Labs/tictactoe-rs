; TicTacToe Client Inno Setup Script
; Run from packaging/windows/
; Expects tictacli.exe to be signed before running ISCC.exe

#ifndef AppVersion
#define AppVersion "0.0.0" ; Fallback if not provided
#endif

[Setup]
AppId={{307CD63A-3554-4320-A9F1-AC6819F3D98B}
AppName=TicTacToe Client
AppVersion={#AppVersion}
AppPublisher=CodeAlchemy-Labs
AppPublisherURL=https://github.com/CodeAlchemy-Labs/tictactoe-rs
DefaultDirName={autopf}\TicTacToe Client
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile=LICENSE.rtf
InfoAfterFile=readme-after-install.txt
SetupIconFile=tictacli.ico
OutputBaseFilename=tictacli-{#AppVersion}-setup
OutputDir=..\..\dist\windows
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked

[Files]
Source: "..\..\target\x86_64-pc-windows-msvc\release\tictacli.exe"; DestDir: "{app}"; Flags: ignoreversion
; Optional Visual C++ Redistributable included if requested
; Source: "vc_redist.x64.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall skipifdoesntexist

[Icons]
Name: "{autoprograms}\TicTacToe Client"; Filename: "{app}\tictacli.exe"
Name: "{autodesktop}\TicTacToe Client"; Filename: "{app}\tictacli.exe"; Tasks: desktopicon

[Run]
; Run the VC++ redistributable installer if it was bundled and is needed
; Filename: "{tmp}\vc_redist.x64.exe"; Parameters: "/install /quiet /norestart"; Check: VCRedistNeedsInstall; Flags: waituntilterminated skipifdoesntexist
Filename: "{app}\tictacli.exe"; Description: "Launch TicTacToe Client"; Flags: nowait postinstall skipifsilent

[Code]
function VCRedistNeedsInstall: Boolean;
var
  Version: String;
begin
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Version', Version) then
  begin
    // Installed
    Result := False;
  end
  else
  begin
    // Not installed
    Result := True;
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  if VCRedistNeedsInstall then
  begin
    MsgBox('Warning: Visual C++ Redistributable is required but not detected. The application may fail to start. Please install it from Microsoft.', mbInformation, MB_OK);
  end;
end;
