#ifndef Versao
    #define Versao "0.1.0"
#endif

[Setup]
AppId={{8F76D37A-1B55-4A87-AEE2-60139BEF8E31}
AppName=Zaga
AppVersion={#Versao}
AppPublisher=levigarciia
AppPublisherURL=https://github.com/levigarciia/Zaga
AppSupportURL=https://github.com/levigarciia/Zaga/issues
DefaultDirName={localappdata}\Programs\Zaga
DefaultGroupName=Zaga
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.18362
OutputDir=..\dist
OutputBaseFilename=ZagaSetup-{#Versao}-x64
SetupIconFile=..\assets\marca\zaga.ico
UninstallDisplayIcon={app}\zaga.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "portugues"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "atalho"; Description: "Criar atalho na área de trabalho"; Flags: unchecked

[Files]
Source: "..\target\release\zaga.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Zaga"; Filename: "{app}\zaga.exe"
Name: "{autodesktop}\Zaga"; Filename: "{app}\zaga.exe"; Tasks: atalho

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "ZAGA"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\zaga.exe"; Description: "Abrir o Zaga"; Flags: nowait postinstall skipifsilent
