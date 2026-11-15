Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef SRC_EXE
  !error "SRC_EXE is required"
!endif
!ifndef OUTFILE
  !error "OUTFILE is required"
!endif
!ifndef LICENSE_FILE
  !error "LICENSE_FILE is required"
!endif
!ifndef NOTICES_FILE
  !error "NOTICES_FILE is required"
!endif

!define APP_NAME "MS Key"
!define APP_EXE "MSKey.exe"
!define APP_PUBLISHER "Bui Dang Minh"
!define APP_URL "https://github.com/buidangminh23/ms-key"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\MSKey"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

Name "${APP_NAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\MS Key"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
BrandingText "${APP_NAME} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ABORTWARNING
!define MUI_ICON "${BRANDING_DIR}\mskey.ico"
!define MUI_UNICON "${BRANDING_DIR}\mskey.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "${BRANDING_DIR}\installer-header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${BRANDING_DIR}\installer-sidebar.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${BRANDING_DIR}\installer-sidebar.bmp"
!define MUI_INSTFILESPAGE_PROGRESSBAR smooth
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_PARAMETERS "--show"
!define MUI_FINISHPAGE_RUN_TEXT "Mở MS Key ngay"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${LICENSE_FILE}"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Vietnamese"

!macro StopRunningApp
  nsExec::Exec 'taskkill /F /IM ${APP_EXE}'
  nsExec::Exec 'taskkill /F /IM mskey-win.exe'
  Sleep 500
!macroend

Section "MS Key" SecMain
  SectionIn RO
  !insertmacro StopRunningApp
  SetOutPath "$INSTDIR"
  File "/oname=${APP_EXE}" "${SRC_EXE}"
  File "/oname=LICENSE.txt" "${LICENSE_FILE}"
  File "/oname=THIRD-PARTY-NOTICES.txt" "${NOTICES_FILE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\MS Key"
  CreateShortcut "$SMPROGRAMS\MS Key\MS Key.lnk" "$INSTDIR\${APP_EXE}" "--show" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$SMPROGRAMS\MS Key\Gỡ cài đặt MS Key.lnk" "$INSTDIR\Uninstall.exe"

  ReadRegStr $0 HKCU "${RUN_KEY}" "MSKey"
  ${If} $0 != ""
    WriteRegStr HKCU "${RUN_KEY}" "MSKey" '"$INSTDIR\${APP_EXE}"'
  ${EndIf}

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "${APP_URL}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
  !insertmacro StopRunningApp
  DeleteRegValue HKCU "${RUN_KEY}" "MSKey"
  nsExec::Exec 'schtasks /Delete /TN MSKeyAutoStart /F'
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\MS Key\MS Key.lnk"
  Delete "$SMPROGRAMS\MS Key\Gỡ cài đặt MS Key.lnk"
  RMDir "$SMPROGRAMS\MS Key"
  DeleteRegKey HKCU "${UNINST_KEY}"
SectionEnd
