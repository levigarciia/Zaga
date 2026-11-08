#ifndef Versao
    #define Versao "0.2.0"
#endif
#ifndef Binario
    #define Binario "..\target\release\ZAGA.exe"
#endif

[Setup]
AppId={{8F76D37A-1B55-4A87-AEE2-60139BEF8E31}
AppName=ZAGA
AppVersion={#Versao}
AppPublisher=levigarciia
AppPublisherURL=https://github.com/levigarciia/Zaga
AppSupportURL=https://github.com/levigarciia/Zaga/issues
DefaultDirName={localappdata}\Programs\Zaga
DefaultGroupName=ZAGA
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.18362
OutputDir=..\dist
OutputBaseFilename=ZagaSetup-{#Versao}-x64
SetupIconFile=..\assets\marca\zaga.ico
UninstallDisplayIcon={app}\ZAGA.exe
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
Source: "{#Binario}"; DestDir: "{app}"; DestName: "ZAGA.exe"; Flags: ignoreversion
Source: "..\docs\licenses\FortniteReplayDecompressor.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\assets\fonts\Inter.OFL.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\assets\fonts\SpaceGrotesk.OFL.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\ZAGA"; Filename: "{app}\ZAGA.exe"
Name: "{autodesktop}\ZAGA"; Filename: "{app}\ZAGA.exe"; Tasks: atalho

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "ZAGA"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\ZAGA.exe"; Description: "Abrir o ZAGA"; Flags: nowait postinstall skipifsilent
