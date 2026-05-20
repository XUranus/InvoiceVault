# Bundled Windows Dependencies

InvoiceVault Windows releases can bundle Poppler, ImageMagick, and ONNX Runtime
from one release asset.

Create the dependency zip locally:

```powershell
npm run package:win-deps
```

Default input paths:

```text
C:\Program Files\ImageMagick-7.1.2-Q8
C:\popper
src-tauri\resources\win-x86_64\onnxruntime.dll
```

The output is:

```text
dist\invoicevault-dependencies-win-x86_64.zip
```

Zip layout:

```text
win-x86_64/
  ImageMagick/
  poppler/
  onnxruntime.dll
```

Upload the zip to the GitHub Release asset used by the Windows release workflow:

```text
https://github.com/XUranus/InvoiceVault/releases/download/build/invoicevault-dependencies-win-x86_64.zip
```

To use a different URL, set the repository variable `WINDOWS_DEPS_URL`.
