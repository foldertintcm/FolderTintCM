# AGENTS.md — FolderTintCM

Contexto para agentes de código (y personas) que trabajen en este repositorio.
El detalle está en [DEVELOPMENT.md](DEVELOPMENT.md): aquí solo lo que no se
puede saltar. Responde y documenta en **español**; el código y sus
comentarios van en inglés.

## Qué es

Colores para las carpetas de Windows 10 y 11 desde el clic derecho del
Explorador, al estilo del Mac. Rust sobre las API de Windows, sin proceso
residente: cada entrada del menú ejecuta `foldertintcm.exe` una vez y termina.
Dos formas de instalarlo, con el mismo programa:

- **MSI** (`installer/`): menú clásico registrado en `HKCU\Software\Classes`
  (en Windows 11, en «Mostrar más opciones»).
- **Microsoft Store / MSIX** (`msix/`): el menú es una extensión COM
  (`shellext/`, `IExplorerCommand`) y sale en el primer menú de Windows 11.

Proyecto hermano: [ScreenShotCM](https://github.com/screenshotcm/ScreenShotCM)
sigue los mismos patrones; ante la duda, haz lo que hace él.

## Comandos

```powershell
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 test --workspace
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 clippy --workspace --release --all-targets '--' -D warnings
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 fmt --all
pwsh -ExecutionPolicy Bypass ./installer/build.ps1          # dist/FolderTintCM-<versión>.msi
pwsh -ExecutionPolicy Bypass ./msix/build.ps1 [-Layout]     # dist/FolderTintCM-<versión>.msix
```

`cargo` va siempre a través de `scripts/cargo.ps1` (pone llvm-mingw en el
PATH; objetivo `x86_64-pc-windows-gnullvm`, sin Visual Studio). Los
ejecutables son de subsistema gráfico: para esperar a uno desde PowerShell usa
`Start-Process -Wait`.

## Reglas

- **Una sola dependencia**: el crate `windows` (y `windows-core` para
  `#[implement]`). Nada nuevo sin licencia permisiva y sin anotarlo en
  [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
- **La paleta es la única fuente** (`src/palette.rs`): de ella salen los
  iconos, los textos del menú (tablas de cadenas en `build.rs`), el fragmento
  WiX y el submenú de la extensión. Un color se añade solo ahí. Nunca se
  renombra un *slug*: hay carpetas apuntando a `<slug>.ico`.
- **Textos de la interfaz** con `tr("español", "English")`; en recursos RC,
  como `L"…"` con escapes `\xNNNN` (windres estropea el UTF-8).
- **La extensión `shellext/` corre dentro del Explorador**: sin `unwrap`,
  índices ni nada que pueda entrar en pánico; cada error es un HRESULT.
- **Nunca deshacer un icono que no puso FolderTintCM** (`folder::colour_of`),
  ni tocar carpetas del sistema. Solo se escribe en `HKEY_CURRENT_USER` y en la
  carpeta de datos; sin red.
- **El sitio público** (repositorio `foldertintcm.github.io`) no muestra
  detalles internos (rutas, `desktop.ini`, claves de registro).
- Pruebas para todo lo puro; lo que toca Windows se prueba de verdad en
  carpetas temporales que se limpian solas. Al terminar, ningún proceso de
  prueba debe quedar corriendo.

## Git y publicación

- Commits con el correo privado de GitHub (`git config user.email
  26506153+cmonroyp@users.noreply.github.com` en este repositorio): GitHub
  rechaza el correo personal. Rama `main`; no reescribir historia publicada.
- **Versión nueva**: subir `version` en `Cargo.toml`, `shellext/Cargo.toml` y
  `assets/app.manifest` → etiqueta `vX.Y.Z` → GitHub Actions compila el MSI
  oficial → poner su tamaño y SHA-256 en el `index.html` del sitio → crear la
  versión en los dos repositorios con `FolderTintCM-X.Y.Z.msi`, su `.sha256` y
  la copia `FolderTintCM.msi` (enlace permanente).
- **Microsoft Store**: identidad `CarlosM.FolderTintCM` (Store ID
  `9MZCRR9KR8TB`), ya en los valores por defecto de `msix/build.ps1`. Se sube
  sin firmar. Ficha y justificación de `runFullTrust` (máx. 500 caracteres) en
  [msix/store-listing.md](msix/store-listing.md); capturas con
  `msix/store-shots.js`.
