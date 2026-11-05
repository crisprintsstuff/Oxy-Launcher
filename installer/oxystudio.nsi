; OxyStudio per-user Windows NSIS installer.
;
Unicode true
ManifestDPIAware true

!define APPNAME "OxyStudio"
!define VERSION "0.1.0"
!define PUBLISHER "OxyStudio contributors"
!define REGPATH "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyStudio"

!ifndef SOURCE_EXE
  !define SOURCE_EXE "../target/release/oxystudio.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "../dist/${APPNAME}-${VERSION}-Setup.exe"
!endif

Name "${APPNAME}"
Caption "${APPNAME} ${VERSION} Setup"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\OxyStudio"
InstallDirRegKey HKCU "${REGPATH}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "${APPNAME} ${VERSION}"

!include "MUI2.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "oxystudio.ico"
!define MUI_UNICON "oxystudio.ico"

!define MUI_WELCOMEPAGE_TITLE "Welcome to OxyStudio"
!define MUI_WELCOMEPAGE_TEXT "Setup will install OxyStudio ${VERSION} for the current user.$\r$\n$\r$\nOxyStudio is the desktop launcher and hub for Oxy creative applications including OxyBrush and OxyPixel."

!define MUI_LICENSEPAGE_TEXT_TOP "OxyStudio is released under the MIT License."
!define MUI_LICENSEPAGE_TEXT_BOTTOM "If you accept the terms, choose I Agree to continue."

!define MUI_COMPONENTSPAGE_SMALLDESC
!define MUI_COMPONENTSPAGE_TEXT_TOP "The launcher is always installed. A desktop shortcut is optional."

!define MUI_DIRECTORYPAGE_TEXT_TOP "Choose an install folder. This install is for the current user and does not need administrator rights."
!define MUI_DIRECTORYPAGE_TEXT_DESTINATION "Install OxyStudio in:"

!define MUI_FINISHPAGE_TITLE "OxyStudio is installed"
!define MUI_FINISHPAGE_TEXT "OxyStudio ${VERSION} is ready to use.$\r$\n$\r$\nLaunch it now to manage and launch OxyBrush and OxyPixel."
!define MUI_FINISHPAGE_TEXT_LARGE
!define MUI_FINISHPAGE_RUN "$INSTDIR\oxystudio.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch OxyStudio"
!define MUI_FINISHPAGE_NOREBOOTSUPPORT

!define MUI_UNCONFIRMPAGE_TEXT_TOP "This removes OxyStudio for the current user, including Start menu shortcuts."
!define MUI_UNCONFIRMPAGE_TEXT_LOCATION "Installed in:"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "../LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

VIProductVersion "0.1.0.0"
VIFileVersion "0.1.0.0"
VIAddVersionKey "ProductName" "OxyStudio"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "FileDescription" "OxyStudio installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "InternalName" "OxyStudio Setup"
VIAddVersionKey "OriginalFilename" "${APPNAME}-${VERSION}-Setup.exe"
VIAddVersionKey "LegalCopyright" "Copyright (C) 2026 OxyStudio contributors. MIT License."

Section "OxyStudio" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${SOURCE_EXE}"
  File "oxystudio.ico"
  File "../README.md"
  File "../LICENSE"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\OxyStudio"
  CreateShortcut "$SMPROGRAMS\OxyStudio\OxyStudio.lnk" "$INSTDIR\oxystudio.exe" "" "$INSTDIR\oxystudio.ico" 0
  CreateShortcut "$SMPROGRAMS\OxyStudio\Uninstall OxyStudio.lnk" "$INSTDIR\Uninstall.exe"

  WriteRegStr HKCU "${REGPATH}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${REGPATH}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${REGPATH}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${REGPATH}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${REGPATH}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
  WriteRegStr HKCU "${REGPATH}" "QuietUninstallString" "$\"$INSTDIR\Uninstall.exe$\" /S"
  WriteRegStr HKCU "${REGPATH}" "DisplayIcon" "$INSTDIR\oxystudio.ico"
  WriteRegDWORD HKCU "${REGPATH}" "NoModify" 1
  WriteRegDWORD HKCU "${REGPATH}" "NoRepair" 1
  SectionGetSize ${SecMain} $0
  WriteRegDWORD HKCU "${REGPATH}" "EstimatedSize" $0
SectionEnd

Section /o "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\OxyStudio.lnk" "$INSTDIR\oxystudio.exe" "" "$INSTDIR\oxystudio.ico" 0
SectionEnd

LangString DESC_MAIN ${LANG_ENGLISH} "The OxyStudio desktop launcher and hub."
LangString DESC_DESKTOP ${LANG_ENGLISH} "Also place a shortcut on the desktop."

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} $(DESC_MAIN)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_DESKTOP)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\oxystudio.exe"
  Delete "$INSTDIR\oxystudio.ico"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\OxyStudio\OxyStudio.lnk"
  Delete "$SMPROGRAMS\OxyStudio\Uninstall OxyStudio.lnk"
  RMDir "$SMPROGRAMS\OxyStudio"
  Delete "$DESKTOP\OxyStudio.lnk"
  DeleteRegKey HKCU "${REGPATH}"
  RMDir "$INSTDIR"
SectionEnd
