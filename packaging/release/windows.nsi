Unicode true
!include "MUI2.nsh"
Name "Graphite"
OutFile "@OUTPUT@"
InstallDir "$LOCALAPPDATA\Programs\Graphite"
RequestExecutionLevel user
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "PortugueseBR"

Section "Graphite"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "@BUNDLE@\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Graphite"
  CreateShortcut "$SMPROGRAMS\Graphite\Graphite.lnk" "$INSTDIR\graphite.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite" "DisplayName" "Graphite"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite" "DisplayVersion" "@VERSION@"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$INSTDIR\graphite.exe"
  Delete "$INSTDIR\code.exe"
  Delete "$INSTDIR\graphite-extension-host.exe"
  Delete "$INSTDIR\LICENSE"
  RMDir /r "$INSTDIR\code-oss"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Graphite\Graphite.lnk"
  RMDir "$SMPROGRAMS\Graphite"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite"
SectionEnd
