# FolderTint

**Colores para las carpetas de Windows 10 y 11, como en el Mac.** Clic
derecho en una carpeta → **FolderTint** → elige un color. Gratis, sin cuenta,
sin publicidad y sin conexión a internet.

![El selector de color de FolderTint](https://foldertint.github.io/assets/img/es/picker-dark.png)

**[Instalador MSI](https://github.com/foldertint/foldertint.github.io/releases/latest/download/FolderTint.msi)** ·
[Sitio web](https://foldertint.github.io/) ·
[Privacidad](https://foldertint.github.io/privacy.html) ·
[Soporte](https://github.com/foldertint/foldertint.github.io/issues) ·
[English](#english)

## Instalar

Descarga el instalador MSI y ábrelo. Se instala para tu usuario, sin pedir
administrador. El MSI no va firmado: Windows SmartScreen puede avisar al
abrirlo (*Más información → Ejecutar de todas formas*). Cada versión, en
[Releases](https://github.com/foldertint/foldertint.github.io/releases),
publica su SHA-256 para comprobar que el archivo es el original.

Se desinstala desde *Configuración → Aplicaciones*: cada carpeta que
coloreaste vuelve a su icono normal.

## Qué hace

- **13 colores** de la paleta de Apple: Rojo, Naranja, Amarillo, Verde, Menta,
  Turquesa, Celeste, Azul, Índigo, Morado, Rosa, Marrón y Gris.
- **Personalizado…**: su propio selector, con vista previa de la carpeta en
  vivo, cuadro de saturación y brillo, barra de tono, campo HEX y la paleta y
  tus colores recientes a un clic. Claro u oscuro, como Windows.
- **Tus 6 últimos colores** aparecen también en el menú.
- **Quitar color** deja la carpeta como estaba, y nunca toca un icono que no
  puso FolderTint.
- Sobre una carpeta, en el fondo de una carpeta abierta o con varias
  carpetas seleccionadas (un solo selector para todas).
- En **Windows 11** el menú está en *Mostrar más opciones* (`Mayús+F10`).

## Se queda

El color se guarda dentro de la carpeta, en un `desktop.ini` oculto que
Windows lee cada vez que la muestra (lo mismo que *Propiedades → Personalizar
→ Cambiar icono*). Sigue ahí al reiniciar o actualizar Windows y viaja con la
carpeta si la mueves o la renombras. No hay nada corriendo en segundo plano.

## Desarrollo

Rust sobre las API de Windows, una sola dependencia (`windows`). Ver
[DEVELOPMENT.md](DEVELOPMENT.md) para compilar, probar y empaquetar.

## Licencia

Código bajo [GPL-3.0-or-later](LICENSE). El nombre y el icono no están
incluidos en esa licencia: ver [TRADEMARKS.md](TRADEMARKS.md). Licencias de
terceros en [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md); seguridad en
[SECURITY.md](SECURITY.md).

---

## English

**Colours for Windows 10 and 11 folders, as on a Mac.** Right-click a folder
→ **FolderTint** → pick a colour. Free, no account, no ads and no internet
connection.

**[MSI installer](https://github.com/foldertint/foldertint.github.io/releases/latest/download/FolderTint.msi)** ·
[Website](https://foldertint.github.io/) ·
[Privacy](https://foldertint.github.io/privacy.html) ·
[Support](https://github.com/foldertint/foldertint.github.io/issues)

### Install

Download the MSI installer and open it. It installs for your user, without
administrator rights. The MSI is not signed: Windows SmartScreen may warn when
you open it (*More info → Run anyway*). Each release publishes its SHA-256 to
check the file is the original. Uninstall from *Settings → Apps*: every folder
you coloured gets its usual icon back.

### What it does

- **13 colours** from Apple's palette.
- **Custom…**: its own picker, with a live folder preview, a saturation and
  value square, a hue bar, a HEX field, and the palette and your recent colours
  one click away. Light or dark, like Windows.
- **Your last 6 colours** show up in the menu too.
- **Remove colour** puts the folder back as it was, and never touches an icon
  FolderTint did not set.
- On a folder, on the background of an open folder, or on several selected
  folders at once (one picker for all of them).
- On **Windows 11** the menu is under *Show more options* (`Shift+F10`).

### It stays

The colour is kept inside the folder, in a hidden `desktop.ini` Windows reads
every time it shows it (the same as *Properties → Customize → Change Icon*).
It survives restarts and Windows updates and travels with the folder when you
move or rename it. Nothing runs in the background.

### Licence

Code under [GPL-3.0-or-later](LICENSE). The name and icon are not covered by
that licence: see [TRADEMARKS.md](TRADEMARKS.md).
