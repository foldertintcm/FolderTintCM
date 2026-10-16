# Desarrollo de FolderTint

Mismo entorno que ScreenShotCM: Rust con el objetivo
`x86_64-pc-windows-gnullvm` y llvm-mingw (sin Visual Studio), WiX 5.0.2 para
el MSI. Ver los pasos de instalación de las herramientas en
`PRY.ScreenShotCM/app/DEVELOPMENT.md`.

```powershell
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 test
pwsh -ExecutionPolicy Bypass ./scripts/cargo.ps1 clippy --release --all-targets '--' -D warnings
pwsh -ExecutionPolicy Bypass ./installer/build.ps1   # → dist/FolderTint-<versión>.msi
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
que el menú lee con `MUIVerb=@foldertint.exe,-id`.

### Decisiones

- **`%LOCALAPPDATA%` en `desktop.ini`**: la carpeta coloreada se ve igual
  para cualquier usuario que tenga FolderTint (unidades compartidas o USB).
- **Menú clásico en `HKCU\Software\Classes\Directory`** (y `\Background`),
  declarado en el MSI: se instala y desinstala sin código propio.
- **Sin tabla de cadenas neutra (0,0)**: el cargador la prefería y ocultaba la
  española en un Windows es-ES.
- **El instalador no borra `HKCU\Software\FolderTint` con
  `RemoveRegistryKey`**: también se ejecutaba en la mitad de desinstalación de
  una actualización y perdía la lista de carpetas. La limpia `--uninstall`.

## Órdenes

| Orden | Uso |
|---|---|
| `foldertint.exe` | Muestra cómo se usa |
| `--apply <color o #rrggbb> <carpeta>` | Lo ejecuta el menú (paleta o color propio) |
| `--pick <carpeta>` | «Personalizado…»: selector de color |
| `--clear <carpeta>` | Lo ejecuta el menú |
| `--uninstall` | Lo ejecuta el MSI al desinstalar |
| `--export-installer-assets <dir>` | Lo ejecuta `installer/build.ps1` |
| `--export-brand <dir>` | Iconos PNG para el sitio web |

## Colores propios

- El icono de un color propio se dibuja la primera vez que se usa, en
  `%LOCALAPPDATA%\FolderTint\custom\<rrggbb>.ico` (escritura atómica: varias
  carpetas a la vez lanzan varios procesos).
- Los recientes se guardan en `HKCU\Software\FolderTint\Custom\Recent` y el
  programa escribe sus entradas `50`–`55` bajo las claves del menú (solo si el
  MSI instaló el menú). El MSI no las conoce: `--uninstall` las borra, y por
  eso se ejecuta antes de `RemoveRegistryValues`.
- Con varias carpetas seleccionadas Explorer lanza un proceso por carpeta: un
  mutex (`Local\FolderTint.Pick`) deja que el primero muestre el selector y
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

- `foldertint.exe --export-brand <carpeta>` escribe el icono (`icon-44.png`,
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

`installer/build.ps1` deja también `dist/FolderTint.msi`, el nombre fijo que
se sube a cada versión del repositorio del sitio
(`foldertint/foldertint.github.io`), para el enlace permanente
`releases/latest/download/FolderTint.msi`.

## Pendiente

- Menú de primer nivel en Windows 11 (sin *Mostrar más opciones*): requiere
  un `IExplorerCommand` en una DLL COM y un paquete con identidad (MSIX o
  paquete disperso firmado).
- Publicación: la organización `foldertint` en GitHub con sus dos
  repositorios, la primera versión en Releases, firma de código y
  Microsoft Store.
