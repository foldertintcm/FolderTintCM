# Desarrollo de FolderTintCM

Mismo entorno que ScreenShotCM: Rust con el objetivo
`x86_64-pc-windows-gnullvm` y llvm-mingw (sin Visual Studio), WiX 5.0.2 para
el MSI. Ver los pasos de instalación de las herramientas en
`PRY.ScreenShotCM/app/DEVELOPMENT.md`.

```powershell
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 test
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 clippy --release --all-targets '--' -D warnings
pwsh -ExecutionPolicy Bypass ./installer/build.ps1   # → dist/FolderTintCM-<versión>.msi
```

## Arquitectura

Un solo ejecutable sin proceso residente y una sola dependencia (`windows`,
MIT OR Apache-2.0). La paleta es la única fuente de verdad: de ella salen los
iconos, los textos del menú y las entradas del instalador.

| Módulo | Qué hace | Windows |
|---|---|---|
| `palette.rs` | Los 13 colores (sistema de Apple), nombres es/en, ids de recursos | no |
| `art.rs` | Dibuja la carpeta con campos de distancia (nítida de 16 a 256 px) | no |
| `ico.rs` | Codifica `.ico` (DIB + PNG propio con deflate mínimo) | no |
| `cli.rs` | Analiza la línea de órdenes, estricto | no |
| `paths.rs` | Dónde están los iconos y cómo los nombra `desktop.ini` | no |
| `folder.rs` | Pone y quita el icono con `SHGetSetFolderCustomSettings` | sí |
| `custom.rs` | Colores propios: iconos al vuelo, recientes en el menú, un selector por selección múltiple | sí |
| `picker/` | Selector propio: `hsv`, `layout` y `paint` (Rust puro, probados) y `window` (Win32 + GDI) | parcial |
| `reg.rs` | Operaciones mínimas de registro en HKCU | sí |
| `ledger.rs` | Registro de carpetas coloreadas, para restaurarlas al desinstalar | sí |
| `installer.rs` | Exporta iconos y el fragmento WiX del menú | no |
| `main.rs` | Une todo; mensajes de error | sí |

`build.rs` incluye `palette.rs`, `art.rs` e `ico.rs` (Rust puro) para dibujar
el icono de la aplicación y generar las tablas de cadenas en español e inglés
que el menú lee con `MUIVerb=@foldertintcm.exe,-id`.

### Decisiones

- **`%LOCALAPPDATA%` en `desktop.ini`**: la carpeta coloreada se ve igual
  para cualquier usuario que tenga FolderTintCM (unidades compartidas o USB).
- **Menú clásico en `HKCU\Software\Classes\Directory`** (y `\Background`),
  declarado en el MSI: se instala y desinstala sin código propio.
- **Sin tabla de cadenas neutra (0,0)**: el cargador la prefería y ocultaba la
  española en un Windows es-ES.
- **El instalador no borra `HKCU\Software\FolderTintCM` con
  `RemoveRegistryKey`**: también se ejecutaba en la mitad de desinstalación de
  una actualización y perdía la lista de carpetas. La limpia `--uninstall`.

## Órdenes

| Orden | Uso |
|---|---|
| `foldertintcm.exe` | Muestra cómo se usa |
| `--apply <color o #rrggbb> <carpeta>` | Lo ejecuta el menú (paleta o color propio) |
| `--pick <carpeta>` | «Personalizado…»: selector de color |
| `--clear <carpeta>` | Lo ejecuta el menú |
| `--uninstall` | Lo ejecuta el MSI al desinstalar |
| `--export-installer-assets <dir>` | Lo ejecuta `installer/build.ps1` |
| `--export-brand <dir>` | Iconos PNG para el sitio web |

## Colores propios

- El icono de un color propio se dibuja la primera vez que se usa, en
  `%LOCALAPPDATA%\FolderTintCM\custom\<rrggbb>.ico` (escritura atómica: varias
  carpetas a la vez lanzan varios procesos).
- Los recientes se guardan en `HKCU\Software\FolderTintCM\Custom\Recent` y el
  programa escribe sus entradas `50`–`55` bajo las claves del menú (solo si el
  MSI instaló el menú). El MSI no las conoce: `--uninstall` las borra, y por
  eso se ejecuta antes de `RemoveRegistryValues`.
- Con varias carpetas seleccionadas Explorer lanza un proceso por carpeta: un
  mutex (`Local\FolderTintCM.Pick`) deja que el primero muestre el selector y
  anote la respuesta (`LastPick`, con su hora); los demás, lanzados antes de
  esa respuesta, la reutilizan. Un solo diálogo, el mismo color para todas.
- Las cadenas del recurso se escriben como `L"…"` con escapes `\xNNNN`:
  windres leía el UTF-8 como Windows-1252 («MarrÃ³n»).

## Selector de color

`picker/` sustituye al `ChooseColor` de Windows, que además se veía
descuadrado al 150 %. La ventana se pinta entera en software (`paint.rs`,
distancias con signo como las carpetas) sobre un DIB, con el texto encima por
GDI y un único control EDIT real para el HEX. Escala con el DPI del monitor
(`WM_DPICHANGED`), pinta la barra de título con DWM en el tono de la ventana y
sigue el tema de las aplicaciones de Windows. En una compilación de depuración,
`FT_THEME=light|dark` fuerza el tema para revisar los dos.

Si el color elegido es uno de la paleta, se aplica como tal (su icono
instalado, sin duplicarlo en los recientes).

## Herramientas de mantenimiento

- `foldertintcm.exe --export-brand <carpeta>` escribe el icono (`icon-44.png`,
  `icon-150.png`, `icon-256.png`) y las 13 carpetas (`folders/<color>.png`)
  para el sitio web, del mismo dibujo que instala.
- Una compilación de depuración acepta, para fotografiar el selector sin tocar
  la configuración de Windows ni la del usuario:
  `FT_THEME=light|dark`, `FT_LANG=es|en`, `FT_SCALE=2` (nitidez en un monitor
  al 100 %) y `FT_RECENT=2fb5a5,ff7a59,…` (colores recientes inventados). La
  versión publicada los ignora.

## Integración continua y publicación

Como en ScreenShotCM: cada envío a `main` y cada etiqueta `v*` compilan,
prueban, pasan clippy y empaquetan el MSI en GitHub Actions
([build.yml](.github/workflows/build.yml)), con las herramientas fijadas por
versión y hash y las acciones fijadas por commit. Dependabot vigila las
acciones y el crate.

`installer/build.ps1` deja también `dist/FolderTintCM.msi`, el nombre fijo que
se sube a cada versión del repositorio del sitio
(`foldertintcm/foldertintcm.github.io`), para el enlace permanente
`releases/latest/download/FolderTintCM.msi`.

## Microsoft Store (MSIX)

```powershell
pwsh -ExecutionPolicy Bypass ./msix/build.ps1            # dist/FolderTintCM-<versión>.msix
pwsh -ExecutionPolicy Bypass ./msix/build.ps1 -Layout    # además, para probarlo instalado
Add-AppxPackage -Register target/layout/<nombre completo>/AppxManifest.xml   # Modo de desarrollador
```

Identidad reservada en Partner Center: `CarlosM.FolderTintCM` (Store ID
`9MZCRR9KR8TB`), con los valores por defecto de `msix/build.ps1`. Se sube sin
firmar: la Store lo firma. Necesita `makeappx.exe`
(Microsoft.Windows.SDK.BuildTools, como ScreenShotCM).

- Un paquete no puede registrar el menú en el registro, así que declara una
  extensión del Explorador: `shellext/` es una DLL COM en Rust
  (`IExplorerCommand`) con el mismo submenú, que en Windows 11 sale en el
  primer menú. Cada entrada ejecuta `foldertintcm.exe`, como el MSI.
- El programa sabe que corre desde un paquete por el nombre de su carpeta
  (`package.rs`), igual que la extensión, que no tiene identidad de paquete.
  Guarda iconos y colores recientes en la carpeta de datos del paquete
  (`%LOCALAPPDATA%\Packages\<familia>\LocalCache\FolderTintCM`): no cambia
  con las actualizaciones y Windows la borra al desinstalar, y entonces las
  carpetas vuelven solas al icono normal. No escribe en el registro.
- La DLL corre dentro del Explorador: sin `unwrap` ni índices que puedan
  fallar; cada error es un HRESULT.
- Ficha: `msix/store-listing.md`. Capturas (1920 × 1080, es/en):
  `node msix/store-shots.js <msedge.exe> <url de PRY.FolderTintCM> dist/store`
  sirviendo la carpeta del proyecto por HTTP.

## Pendiente

- Menú de primer nivel en Windows 11 también para el MSI (hoy solo lo tiene
  el paquete de la Store): requiere registrar la extensión con un paquete
  disperso firmado.
- Firma de código del MSI (SignPath u otro), para que SmartScreen no avise.
