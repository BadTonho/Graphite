Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "Sections.nsh"
Name "Graphite"
OutFile "@OUTPUT@"
InstallDir "$LOCALAPPDATA\Programs\Graphite"
RequestExecutionLevel user
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "PortugueseBR"

Var RuntimeStage

LangString VSIXName ${LANG_ENGLISH} "VSIX Plugins"
LangString VSIXName ${LANG_PORTUGUESEBR} "Plugins VSIX"
LangString VSIXDescription ${LANG_ENGLISH} "Installs Node.js and the Code OSS host that runs VSIX extensions with executable code."
LangString VSIXDescription ${LANG_PORTUGUESEBR} "Instala o Node.js e o host Code OSS que executa extensões VSIX com código."
LangString EditorDescription ${LANG_ENGLISH} "Installs the Graphite editor and its command-line tools."
LangString EditorDescription ${LANG_PORTUGUESEBR} "Instala o editor Graphite e suas ferramentas de linha de comando."
LangString CopilotDescription ${LANG_ENGLISH} "Installs the Copilot SDK modules for the Code OSS runtime."
LangString CopilotDescription ${LANG_PORTUGUESEBR} "Instala os módulos do Copilot SDK para o runtime Code OSS."
LangString FoundryDescription ${LANG_ENGLISH} "Installs the Foundry Local SDK module for the Code OSS runtime."
LangString FoundryDescription ${LANG_PORTUGUESEBR} "Instala o módulo Foundry Local SDK para o runtime Code OSS."
LangString InstallError ${LANG_ENGLISH} "Graphite setup could not write the installation files. Close Graphite and try again."
LangString InstallError ${LANG_PORTUGUESEBR} "A instalação do Graphite não conseguiu gravar os arquivos. Feche o Graphite e tente novamente."
LangString RestoreError ${LANG_ENGLISH} "Setup could not restore the previous Code OSS runtime. The backup is in $RuntimeStage\previous."
LangString RestoreError ${LANG_PORTUGUESEBR} "O instalador não conseguiu restaurar o runtime Code OSS anterior. A cópia está em $RuntimeStage\previous."

Section "Graphite" Editor
  SectionIn RO
  SetShellVarContext current
  ClearErrors
  SetOutPath "$INSTDIR"
  IfErrors 0 +2
    Call FailInstall

  ; Stage the selected runtime on the same volume as its final destination.
  GetTempFileName $RuntimeStage "$INSTDIR"
  IfErrors 0 +2
    Call FailInstall
  Delete "$RuntimeStage"
  CreateDirectory "$RuntimeStage"
  IfErrors 0 +2
    Call FailInstall
SectionEnd

Section "$(VSIXName)" VSIX
  ClearErrors
  SetOutPath "$RuntimeStage\code-oss"
  File /r "@COMPONENTS@\vsix\*"
  IfErrors 0 +2
    Call FailInstall
SectionEnd

Section "Copilot SDK" Copilot
  ClearErrors
  SetOutPath "$RuntimeStage\code-oss"
  File /r "@COMPONENTS@\copilot\*"
  IfErrors 0 +2
    Call FailInstall
SectionEnd

Section "Foundry Local SDK" Foundry
  ClearErrors
  SetOutPath "$RuntimeStage\code-oss"
  File /r "@COMPONENTS@\foundry\*"
  IfErrors 0 +2
    Call FailInstall
SectionEnd

Section "-Install"
  ClearErrors
  SetOutPath "$INSTDIR"
  File /r "@BUNDLE@\*"
  IfErrors 0 +2
    Call FailInstall

  Call ReplaceRuntime
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Graphite"
  CreateShortcut "$SMPROGRAMS\Graphite\Graphite.lnk" "$INSTDIR\graphite.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite" "DisplayName" "Graphite"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite" "DisplayVersion" "@VERSION@"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${Editor} $(EditorDescription)
  !insertmacro MUI_DESCRIPTION_TEXT ${VSIX} $(VSIXDescription)
  !insertmacro MUI_DESCRIPTION_TEXT ${Copilot} $(CopilotDescription)
  !insertmacro MUI_DESCRIPTION_TEXT ${Foundry} $(FoundryDescription)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInit
  IfSilent 0 done
  ${GetParameters} $0
  ${GetOptions} $0 "/NO-VSIX" $1
  IfErrors check_copilot
    !insertmacro UnselectSection ${VSIX}
  check_copilot:
  ${GetOptions} $0 "/NO-COPILOT" $1
  IfErrors check_foundry
    !insertmacro UnselectSection ${Copilot}
  check_foundry:
  ${GetOptions} $0 "/NO-FOUNDRY" $1
  IfErrors done
    !insertmacro UnselectSection ${Foundry}
  done:
FunctionEnd

Function ReplaceRuntime
  IfFileExists "$INSTDIR\code-oss\." 0 install_selected
    ClearErrors
    Rename "$INSTDIR\code-oss" "$RuntimeStage\previous"
    IfErrors 0 install_selected
      Call FailInstall

  install_selected:
    IfFileExists "$RuntimeStage\code-oss\." 0 finish
    ClearErrors
    Rename "$RuntimeStage\code-oss" "$INSTDIR\code-oss"
    IfErrors 0 finish
    IfFileExists "$RuntimeStage\previous\." 0 failed
      ClearErrors
      Rename "$RuntimeStage\previous" "$INSTDIR\code-oss"
      IfErrors 0 failed
      MessageBox MB_OK|MB_ICONSTOP "$(RestoreError)" /SD IDOK
      DetailPrint "$(RestoreError)"
      SetErrorLevel 1
      Abort
  failed:
    Call FailInstall
  finish:
    RMDir /r "$RuntimeStage"
    StrCpy $RuntimeStage ""
FunctionEnd

Function FailInstall
  SetOutPath "$INSTDIR"
  StrCmp $RuntimeStage "" +3
    RMDir /r "$RuntimeStage"
    Delete "$RuntimeStage"
  MessageBox MB_OK|MB_ICONSTOP "$(InstallError)" /SD IDOK
  SetErrorLevel 1
  Abort "$(InstallError)"
FunctionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$INSTDIR\graphite.exe"
  Delete "$INSTDIR\code.exe"
  Delete "$INSTDIR\graphite-extension-host.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  RMDir /r "$INSTDIR\licenses"
  RMDir /r "$INSTDIR\code-oss"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Graphite\Graphite.lnk"
  RMDir "$SMPROGRAMS\Graphite"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Graphite"
SectionEnd
