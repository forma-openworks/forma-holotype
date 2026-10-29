; Установщик Forma: Holotype для Inno Setup 6.7+.
; Собирает его installer/build.ps1: он передаёт версию из Cargo.toml, папку с release-сборкой
; (статический CRT — Visual C++ Redistributable не нужен), иконку из build.rs и картинки мастера.
;
; Куда ставить, спрашивает сам мастер: только для себя (%LOCALAPPDATA%\Programs, без прав
; администратора) или для всех (Program Files). Обновление ставится туда же и тем же способом.

#ifndef AppVersion
  #error AppVersion не задан: собирайте установщик через installer/build.ps1
#endif
#ifndef BinDir
  #error BinDir не задан: собирайте установщик через installer/build.ps1
#endif
#ifndef ArtDir
  #error ArtDir не задан: собирайте установщик через installer/build.ps1
#endif
#ifndef IconFile
  #error IconFile не задан: собирайте установщик через installer/build.ps1
#endif
#ifndef OutputDir
  #define OutputDir "..\target\installer"
#endif

#define AppName "Forma: Holotype"
#define AppExe "forma-holotype.exe"
#define AppUrl "https://github.com/forma-openworks/forma-holotype"

[Setup]
; AppId не менять: по нему находится уже установленная копия.
AppId={{2E4DD940-9833-4915-9C95-79C28A6D3BED}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=FORMA
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\Forma\Holotype
DisableProgramGroupPage=yes
DisableWelcomePage=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=Forma-Holotype-Setup-{#AppVersion}
SetupIconFile={#IconFile}
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
; Светлый или тёмный мастер — как в Windows; картинки для каждой темы свои.
WizardStyle=modern dynamic windows11
WizardImageFile={#ArtDir}\large-light-*.png
WizardImageFileDynamicDark={#ArtDir}\large-dark-*.png
WizardSmallImageFile={#ArtDir}\small-*.png
WizardSmallImageFileDynamicDark={#ArtDir}\small-*.png
Compression=lzma2/ultra64
SolidCompression=yes
LZMAUseSeparateProcess=yes
; PATH меняется — Проводник и новые терминалы увидят его без перезахода.
ChangesEnvironment=yes
; Открытый Holotype мешает обновить .exe: мастер предложит закрыть его сам.
CloseApplications=yes
RestartApplications=no
; Язык спрашиваем всегда: Windows бывает на английском у тех, кому удобнее по-русски.
ShowLanguageDialog=yes

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.AppComment=Notes, study notes and documentation
ru.AppComment=Заметки, конспекты и документация
en.TasksIntegration=Integration:
ru.TasksIntegration=Интеграция:
en.TaskContextMenu=Add "Open as vault in Holotype" to the folder menu in Explorer
ru.TaskContextMenu=Добавить «Открыть как хранилище в Holotype» в меню папок Проводника
en.TaskPath=Add forma-holotype to PATH (to start it from a terminal)
ru.TaskPath=Добавить forma-holotype в PATH (запуск из терминала)
en.OpenAsVault=Open as vault in Holotype
ru.OpenAsVault=Открыть как хранилище в Holotype

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "contextmenu"; Description: "{cm:TaskContextMenu}"; GroupDescription: "{cm:TasksIntegration}"
Name: "addtopath"; Description: "{cm:TaskPath}"; GroupDescription: "{cm:TasksIntegration}"

[Files]
Source: "{#BinDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; В «Пуске» — папка Forma: в ней будут и другие программы семейства.
Name: "{autoprograms}\Forma\Holotype"; Filename: "{app}\{#AppExe}"; Comment: "{cm:AppComment}"
Name: "{autodesktop}\Holotype"; Filename: "{app}\{#AppExe}"; Comment: "{cm:AppComment}"; Tasks: desktopicon

[Registry]
; «Открыть как хранилище в Holotype» — на папке и на пустом месте внутри неё, как «Open with
; Code» у VS Code. Путь к папке уходит программе аргументом.
Root: HKA; Subkey: "Software\Classes\Directory\shell\FormaHolotype"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenAsVault}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\FormaHolotype"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\FormaHolotype\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\FormaHolotype"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenAsVault}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\FormaHolotype"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\FormaHolotype\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%V"""; Tasks: contextmenu
; Пункт сняли при обновлении — убираем оставшийся от прошлой установки.
Root: HKA; Subkey: "Software\Classes\Directory\shell\FormaHolotype"; ValueType: none; Flags: deletekey; Tasks: not contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\FormaHolotype"; ValueType: none; Flags: deletekey; Tasks: not contextmenu
; Win+R → forma-holotype.
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExe}"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe}"; Flags: uninsdeletekey

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
const
  UserEnvKey = 'Environment';
  SystemEnvKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

{ PATH установки: у пользователя — свой, при установке для всех — системный. }
function EnvRoot: Integer;
begin
  if IsAdminInstallMode then
    Result := HKEY_LOCAL_MACHINE
  else
    Result := HKEY_CURRENT_USER;
end;

function EnvKey: string;
begin
  if IsAdminInstallMode then
    Result := SystemEnvKey
  else
    Result := UserEnvKey;
end;

{ Запись PATH как ключ для сравнения: без пробелов по краям, без \ на конце, без регистра. }
function PathKey(const Dir: string): string;
begin
  Result := Uppercase(RemoveBackslashUnlessRoot(Trim(Dir)));
end;

{ Отделяет первую запись PATH от остальных. }
function TakeItem(var Rest: string): string;
var
  P: Integer;
begin
  P := Pos(';', Rest);
  if P = 0 then
  begin
    Result := Rest;
    Rest := '';
  end
  else
  begin
    Result := Copy(Rest, 1, P - 1);
    Rest := Copy(Rest, P + 1, Length(Rest));
  end;
end;

function PathHas(const Paths, Dir: string): Boolean;
var
  Rest: string;
begin
  Result := False;
  Rest := Paths;
  while (Rest <> '') and not Result do
    Result := PathKey(TakeItem(Rest)) = PathKey(Dir);
end;

{ PATH без записей, которые указывают на Dir; остальные записи — как были. }
function PathWithout(const Paths, Dir: string): string;
var
  Rest, Item: string;
begin
  Result := '';
  Rest := Paths;
  while Rest <> '' do
  begin
    Item := TakeItem(Rest);
    if PathKey(Item) <> PathKey(Dir) then
    begin
      if Result <> '' then
        Result := Result + ';';
      Result := Result + Item;
    end;
  end;
end;

{ Читает PATH. Переменной ещё нет — пустая строка. Есть, но не читается — False: тогда PATH
  не трогаем вовсе, чтобы не затереть его. }
function ReadPath(var Paths: string): Boolean;
begin
  Paths := '';
  Result := True;
  if RegValueExists(EnvRoot, EnvKey, 'Path') then
    Result := RegQueryStringValue(EnvRoot, EnvKey, 'Path', Paths);
end;

procedure AddToPath(const Dir: string);
var
  Paths: string;
begin
  if not ReadPath(Paths) or PathHas(Paths, Dir) then
    Exit;
  if (Paths <> '') and (Paths[Length(Paths)] <> ';') then
    Paths := Paths + ';';
  RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', Paths + Dir);
end;

procedure RemoveFromPath(const Dir: string);
var
  Paths: string;
begin
  if ReadPath(Paths) and PathHas(Paths, Dir) then
    RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', PathWithout(Paths, Dir));
end;

{ Программы Forma при каждом запуске пишут, где лежит их .exe (%LOCALAPPDATA%\Forma\apps):
  так плагин одной программы открывает файл в другой. После удаления запись, которая
  указывает на эту копию, больше не нужна; запись другой копии (например, сборки из
  исходников) остаётся. }
procedure ForgetAppRecord;
var
  F, Exe: string;
  Lines: TArrayOfString;
  I: Integer;
begin
  F := ExpandConstant('{localappdata}\Forma\apps\holotype.json');
  if not LoadStringsFromFile(F, Lines) then
    Exit;
  Exe := ExpandConstant('{app}\{#AppExe}');
  StringChangeEx(Exe, '\', '\\', True);
  for I := 0 to GetArrayLength(Lines) - 1 do
    if Pos(Uppercase(Exe), Uppercase(Lines[I])) > 0 then
    begin
      DeleteFile(F);
      Exit;
    end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    if WizardIsTaskSelected('addtopath') then
      AddToPath(ExpandConstant('{app}'))
    else
      RemoveFromPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    ForgetAppRecord;
  end;
end;
