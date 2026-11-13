#define RepoRoot GetEnv("GITHUB_WORKSPACE")
#define AppVersion GetEnv("H5JIVE_VERSION")

[Setup]
AppId=H5Jive
AppName=H5Jive
AppVersion={#AppVersion}
AppPublisher=H5Jive contributors
AppPublisherURL=https://github.com/vrtulka23/h5jive
DefaultDirName={localappdata}\Programs\H5Jive
DefaultGroupName=H5Jive
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#RepoRoot}\dist
OutputBaseFilename=h5jive-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\h5jive.exe
WizardStyle=modern

[Files]
Source: "{#RepoRoot}\bundle\h5jive.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\bundle\*.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\bundle\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\bundle\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\bundle\settings\*"; DestDir: "{app}\settings"; Excludes: "overrides.dip"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#RepoRoot}\bundle\settings\overrides.dip"; DestDir: "{app}\settings"; Flags: onlyifdoesntexist uninsneveruninstall

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Icons]
Name: "{group}\H5Jive"; Filename: "{app}\h5jive.exe"
Name: "{autodesktop}\H5Jive"; Filename: "{app}\h5jive.exe"; Tasks: desktopicon
