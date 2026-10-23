; Instalador do Furinar para Windows (Inno Setup 6).
;
; Instala só para o usuário atual, em %LOCALAPPDATA%\Programs\Furinar, sem
; pedir administrador: assim o atualizador embutido consegue trocar o .exe
; sozinho. As configurações ficam em %APPDATA%\Furinar (ver config.rs) e
; sobrevivem à desinstalação.
;
; O atualizador do app baixa este setup e roda com
;   /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /reabrir=1
; (ver updates.rs::reiniciar).
;
; Compilar (a partir da raiz do repositório):
;   iscc /DVersao=1.7.1 /DExe=target\release\furinar.exe /DIcone=caminho\furinar.ico pkg\windows\furinar.iss
; O instalador sai em dist\furinar-windows-x86_64-setup.exe.

#ifndef Versao
  #error Passe a versão: /DVersao=X.Y.Z
#endif
#ifndef Exe
  #define Exe "..\..\target\release\furinar.exe"
#endif

[Setup]
; Identidade fixa: nunca mudar, senão o Windows trata como outro programa
AppId={{E3015E3A-93E8-4BC8-89BF-E37A47D66745}
AppName=Furinar
AppVersion={#Versao}
AppVerName=Furinar {#Versao}
AppPublisher=LokKol17
AppPublisherURL=https://github.com/LokKol17/furinar
AppSupportURL=https://github.com/LokKol17/furinar/issues
AppUpdatesURL=https://github.com/LokKol17/furinar/releases
VersionInfoVersion={#Versao}
DefaultDirName={autopf}\Furinar
DefaultGroupName=Furinar
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\..\dist
OutputBaseFilename=furinar-windows-x86_64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\furinar.exe
UninstallDisplayName=Furinar
; Fecha o Furinar aberto antes de trocar o executável
CloseApplications=yes
RestartApplications=no
LicenseFile=..\..\LICENSE
#ifdef Icone
SetupIconFile={#Icone}
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "furinar.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Furinar"; Filename: "{app}\furinar.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Furinar"; Filename: "{app}\furinar.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\furinar.exe"; Description: "{cm:LaunchProgram,Furinar}"; Flags: nowait postinstall skipifsilent
; Atualização pelo próprio Furinar: roda em silêncio e reabre o app no fim
Filename: "{app}\furinar.exe"; Flags: nowait; Check: DeveReabrir

[Code]
function DeveReabrir: Boolean;
begin
  Result := WizardSilent and (ExpandConstant('{param:reabrir|0}') = '1');
end;
