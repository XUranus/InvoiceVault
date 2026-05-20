!macro IV_RECREATE_SHORTCUT LINK_PATH
  Delete "${LINK_PATH}"
  SetOutPath "$INSTDIR"
  CreateShortcut "${LINK_PATH}" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
  !insertmacro SetLnkAppUserModelId "${LINK_PATH}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $AppStartMenuFolder != ""
    CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
    !insertmacro IV_RECREATE_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${Else}
    !insertmacro IV_RECREATE_SHORTCUT "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}

  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
  ${OrIf} $PassiveMode = 1
  ${OrIf} ${Silent}
    !insertmacro IV_RECREATE_SHORTCUT "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
!macroend
