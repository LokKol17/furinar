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

; FFmpeg opcional (MKV/WebM/AVI e o vídeo no modo imersivo): build
; "essentials" do Gyan, baixado na instalação e conferido pelo SHA-256 que
; ele publica. Vai para {app}\ffmpeg, onde o Furinar procura antes do PATH.
#define FfmpegVersao "9.0.2"
#define FfmpegPasta "ffmpeg-" + FfmpegVersao + "-essentials_build"
#define FfmpegUrl "https://github.com/GyanD/codexffmpeg/releases/download/" + FfmpegVersao + "/" + FfmpegPasta + ".zip"
#define FfmpegSha256 "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"

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
; "Abrir com" do Explorer (associações abaixo)
ChangesAssociations=yes
RestartApplications=no
LicenseFile=..\..\LICENSE
#ifdef Icone
SetupIconFile={#Icone}
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
en.TarefaFfmpeg=Download FFmpeg {#FfmpegVersao} (~115 MB; ~210 MB installed) for MKV/WebM/AVI and the video in the immersive mode
ptbr.TarefaFfmpeg=Baixar o FFmpeg {#FfmpegVersao} (~115 MB; ~210 MB instalado) para MKV/WebM/AVI e o vídeo no modo imersivo
en.GrupoExtras=Extras:
ptbr.GrupoExtras=Extras:
en.ExtraindoFfmpeg=Extracting FFmpeg...
ptbr.ExtraindoFfmpeg=Extraindo o FFmpeg...

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "ffmpeg"; Description: "{cm:TarefaFfmpeg}"; GroupDescription: "{cm:GrupoExtras}"; Flags: unchecked; Check: not FfmpegInstalado

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "furinar.exe"; Flags: ignoreversion

[Registry]
; Aparece em "Abrir com" sem tomar o lugar do player padrão (por usuário, HKCU)
Root: HKA; Subkey: "Software\Classes\Furinar.Audio"; ValueType: string; ValueName: ""; ValueData: "Furinar"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Furinar.Audio\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\furinar.exe,0"
Root: HKA; Subkey: "Software\Classes\Furinar.Audio\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\furinar.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Furinar"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\furinar.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".mp3"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".flac"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".ogg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".wav"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".m4a"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".mp4"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".m4v"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".mov"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".mkv"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".webm"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\furinar.exe\SupportedTypes"; ValueType: string; ValueName: ".avi"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\.mp3\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.flac\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.ogg\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.wav\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.m4a\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.mp4\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.m4v\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.mov\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.mkv\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.webm\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.avi\OpenWithProgids"; ValueType: string; ValueName: "Furinar.Audio"; ValueData: ""; Flags: uninsdeletevalue

[UninstallDelete]
Type: filesandordirs; Name: "{app}\ffmpeg"

[Icons]
Name: "{autoprograms}\Furinar"; Filename: "{app}\furinar.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Furinar"; Filename: "{app}\furinar.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\furinar.exe"; Description: "{cm:LaunchProgram,Furinar}"; Flags: nowait postinstall skipifsilent
; Atualização pelo próprio Furinar: roda em silêncio e reabre o app no fim
Filename: "{app}\furinar.exe"; Flags: nowait; Check: DeveReabrir

[Code]
var
  PaginaDownload: TDownloadWizardPage;

function DeveReabrir: Boolean;
begin
  Result := WizardSilent and (ExpandConstant('{param:reabrir|0}') = '1');
end;

function FfmpegInstalado: Boolean;
begin
  Result := FileExists(AddBackslash(WizardDirValue) + 'ffmpeg\ffmpeg.exe');
end;

procedure InitializeWizard;
begin
  PaginaDownload := CreateDownloadPage(SetupMessage(msgWizardPreparing), SetupMessage(msgPreparingDesc), nil);
end;

// Baixa antes de instalar (com progresso e o SHA-256 conferido). Sem rede ou
// cancelado, o Furinar instala mesmo assim, só sem o FFmpeg. Na atualização
// silenciosa esta página não roda: o FFmpeg já baixado continua em {app}.
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and WizardIsTaskSelected('ffmpeg') then
  begin
    PaginaDownload.Clear;
    PaginaDownload.Add('{#FfmpegUrl}', 'ffmpeg.zip', '{#FfmpegSha256}');
    PaginaDownload.Show;
    try
      try
        PaginaDownload.Download;
      except
        if not PaginaDownload.AbortedByUser then
          SuppressibleMsgBox(AddPeriod(GetExceptionMessage), mbError, MB_OK, IDOK);
      end;
    finally
      PaginaDownload.Hide;
    end;
  end;
end;

// Texto entre aspas simples para o PowerShell
function Aspas(Texto: String): String;
begin
  StringChangeEx(Texto, '''', '''''', True);
  Result := '''' + Texto + '''';
end;

// Só ffmpeg.exe e ffprobe.exe (e a licença) saem do zip
procedure CurStepChanged(CurStep: TSetupStep);
var
  Zip, Temp, Bin, Destino: String;
  Codigo: Integer;
begin
  if CurStep <> ssPostInstall then
    Exit;
  Zip := ExpandConstant('{tmp}\ffmpeg.zip');
  if not FileExists(Zip) then
    Exit;
  Temp := ExpandConstant('{tmp}\ffmpeg');
  Destino := ExpandConstant('{app}\ffmpeg');
  WizardForm.StatusLabel.Caption := CustomMessage('ExtraindoFfmpeg');
  if Exec('powershell.exe', '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Expand-Archive -LiteralPath ' + Aspas(Zip) + ' -DestinationPath ' + Aspas(Temp) + ' -Force"',
          '', SW_HIDE, ewWaitUntilTerminated, Codigo) and (Codigo = 0) then
  begin
    Bin := Temp + '\{#FfmpegPasta}\bin\';
    ForceDirectories(Destino);
    if not (CopyFile(Bin + 'ffmpeg.exe', Destino + '\ffmpeg.exe', False)
            and CopyFile(Bin + 'ffprobe.exe', Destino + '\ffprobe.exe', False)) then
      Log('FFmpeg: falha ao copiar os executáveis');
    CopyFile(Temp + '\{#FfmpegPasta}\LICENSE', Destino + '\LICENSE.txt', False);
  end
  else
    Log('FFmpeg: Expand-Archive falhou (' + IntToStr(Codigo) + ')');
end;
