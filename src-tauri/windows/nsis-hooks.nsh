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

  ; 删除旧版快捷方式
  Delete "$DESKTOP\InvoiceVault.lnk"
  ${If} $AppStartMenuFolder != ""
    Delete "$SMPROGRAMS\$AppStartMenuFolder\InvoiceVault.lnk"
  ${Else}
    Delete "$SMPROGRAMS\InvoiceVault.lnk"
  ${EndIf}

  ; 创建新版快捷方式
  ${If} $AppStartMenuFolder != ""
    CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
    !insertmacro IV_RECREATE_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder\票匣.lnk"
  ${Else}
    !insertmacro IV_RECREATE_SHORTCUT "$SMPROGRAMS\票匣.lnk"
  ${EndIf}

  !insertmacro IV_RECREATE_SHORTCUT "$DESKTOP\票匣.lnk"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\icon.ico"
  ; 清理当前快捷方式
  Delete "$DESKTOP\票匣.lnk"
  ${If} $AppStartMenuFolder != ""
    Delete "$SMPROGRAMS\$AppStartMenuFolder\票匣.lnk"
  ${Else}
    Delete "$SMPROGRAMS\票匣.lnk"
  ${EndIf}
  ; 清理旧版快捷方式
  Delete "$DESKTOP\InvoiceVault.lnk"
  ${If} $AppStartMenuFolder != ""
    Delete "$SMPROGRAMS\$AppStartMenuFolder\InvoiceVault.lnk"
  ${Else}
    Delete "$SMPROGRAMS\InvoiceVault.lnk"
  ${EndIf}
!macroend
