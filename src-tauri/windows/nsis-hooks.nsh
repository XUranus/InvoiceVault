!macro IV_RECREATE_SHORTCUT LINK_PATH
  Delete "${LINK_PATH}"
  SetOutPath "$INSTDIR"
  CreateShortcut "${LINK_PATH}" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\icon.ico" 0
  !insertmacro SetShortcutTarget "${LINK_PATH}" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "${LINK_PATH}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  File /a "/oname=icon.ico" "${INSTALLERICON}"
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\icon.ico$\""

  ${If} $AppStartMenuFolder != ""
    CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
    !insertmacro IV_RECREATE_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${Else}
    !insertmacro IV_RECREATE_SHORTCUT "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}

  !insertmacro IV_RECREATE_SHORTCUT "$DESKTOP\${PRODUCTNAME}.lnk"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\icon.ico"
!macroend
