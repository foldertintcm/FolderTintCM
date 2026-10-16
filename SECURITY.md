# Seguridad · Security

## Reportar una vulnerabilidad

No abras un issue público. Usa **Security → Report a vulnerability** en este
repositorio (aviso privado de GitHub): solo el autor lo ve hasta que haya una
corrección publicada. Incluye la versión, los pasos para reproducirlo y qué
consigue un atacante.

## Reporting a vulnerability

Please do not open a public issue. Use **Security → Report a vulnerability** in
this repository (GitHub private reporting): only the author sees it until a fix
is released. Include the version, the steps to reproduce it and what an
attacker gains.

## Versiones con soporte · Supported versions

Solo la última versión publicada · Only the latest release.

## Superficie · Surface

- FolderTint no abre conexiones de red ni escucha en ningún puerto, y no deja
  ningún proceso en segundo plano.
  FolderTint opens no network connections, listens on no port and leaves no
  process running in the background.
- La línea de órdenes solo acepta órdenes conocidas, colores de la paleta o
  `#rrggbb`, y carpetas que existen; cualquier otra cosa se rechaza.
  The command line accepts only known verbs, palette colours or `#rrggbb`, and
  folders that exist; anything else is refused.
- Solo escribe el `desktop.ini` de la carpeta, a través de la función del
  sistema (`SHGetSetFolderCustomSettings`), y nunca deshace un icono que no
  puso FolderTint.
  It only writes the folder's `desktop.ini`, through the system function
  (`SHGetSetFolderCustomSettings`), and never undoes an icon FolderTint did not
  set.
- Se ejecuta con los permisos del usuario, sin administrador, y solo escribe
  en `HKEY_CURRENT_USER` y `%LOCALAPPDATA%`.
  It runs with the user's permissions, without elevation, and writes only to
  `HKEY_CURRENT_USER` and `%LOCALAPPDATA%`.
