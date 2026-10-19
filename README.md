# Analizador de disco

Escanea el disco de una PC y separa **lo que creó una persona** (fotos, videos, documentos,
grabaciones, proyectos) de lo que es **sistema o programas**. Sirve para decidir qué conservar
y qué borrar antes de reciclar una máquina, por ejemplo para convertirla en un servidor Linux.

- Un solo ejecutable, sin instalar nada (Windows x64 y Linux x64).
- **Solo lectura**: el escaneo nunca modifica ni borra nada.
- Recorrido en paralelo y medición de velocidad (archivos/s), para comparar HDD y SSD.

## Uso

```sh
analizador ver C:\                      # interfaz de dos paneles
analizador ver ~ --iconos emoji         # íconos: simples (default), emoji o nerd
analizador escanear C:\                 # resumen en texto, para leer o redirigir a un archivo
analizador escanear /media/pendrive     # un pendrive o disco externo
analizador escanear ~ --top 10          # los 10 más pesados de cada categoría
analizador escanear D:\ --hilos 2       # menos hilos: a veces rinde más en un HDD
analizador informe C:\ --salida D:\     # informe Markdown + CSV (muestra antes qué va a escribir)
analizador demo                         # crea una carpeta de prueba y la escanea
analizador demo --ver                   # lo mismo, en la interfaz
```

## La interfaz

A la izquierda el disco, como árbol navegable; a la derecha, lo que hay en la carpeta seleccionada,
clasificado. Los dos paneles van juntos: al moverte por la izquierda, la derecha se recalcula.

```
 analizador  D:\demo  · usuario 15,5 MB · revisar 660,0 KB · excluido 6
╭ ► Disco · D:\demo ────────────────────────────────────────╮╭ Clasificado · Descargas ──────────────────────────────╮
│► Descargas                        9,2 MB ██████░░░░  57,0%││ 1 Categorías │ 2 Proyectos │ 3 Revisar │ 4 Excluido   │
│► Videos                           2,0 MB █░░░░░░░░░  12,4%││                                                       │
│► DCIM                             1,9 MB █░░░░░░░░░  11,7%││■ Fotos                     0 B       0 ░░░░░░░░   0,0%│
│► Música                           1,3 MB █░░░░░░░░░   7,9%││■ Videos                    0 B       0 ░░░░░░░░   0,0%│
│► Imágenes                       670,0 KB ░░░░░░░░░░   4,1%││■ Música                    0 B       0 ░░░░░░░░   0,0%│
│♦ AppData  (revisar)             660,0 KB ░░░░░░░░░░   4,0%││■ Grabaciones de voz        0 B       0 ░░░░░░░░   0,0%│
│► Documentos                     217,0 KB ░░░░░░░░░░   1,3%││■ Documentos                0 B       0 ░░░░░░░░   0,0%│
│► Grabaciones                    200,0 KB ░░░░░░░░░░   1,2%││■ Proyectos/código          0 B       0 ░░░░░░░░   0,0%│
│► Proyectos                       34,0 KB ░░░░░░░░░░   0,2%││■ Comprimidos            1,5 MB       2 █░░░░░░░  16,3%│
│► WhatsApp                        30,0 KB ░░░░░░░░░░   0,2%││■ Instaladores           7,7 MB       3 ███████░  83,7%│
│► Varios                           4,0 KB ░░░░░░░░░░   0,0%││■ Otros                     0 B       0 ░░░░░░░░   0,0%│
│■ LEEME-demo.txt                     55 B ░░░░░░░░░░   0,0%││                                                       │
│× $Recycle.Bin  (sistema)                                  ││                                                       │
│× Program Files  (sistema)                                 ││                                                       │
│× Windows  (sistema)                                       ││Nada de esta categoría acá.                            │
│× pagefile.sys  (sistema)                                  ││Además, para revisar: 0 B                              │
╰──────────────────────────────────── 16,1 MB · 30 archivos ╯╰───────────────────────────────────────────────────────╯
 0,00 s · 16.073 arch/s  ↑↓ mover  Enter entrar  ⌫ subir  Tab panel  1-4 vista  e exportar  q salir
```

| Tecla | Panel del disco | Panel clasificado |
|---|---|---|
| `↑` `↓` `PgUp` `PgDn` `Inicio` `Fin` | moverse | moverse |
| `Enter` / `→` | entrar a la carpeta | ver los archivos de la categoría, de mayor a menor; en un archivo o grupo, saltar a su carpeta en el disco |
| `⌫` / `←` | subir | volver a las categorías / cambiar de vista |
| `Tab` | cambiar de panel | cambiar de panel |
| `1`–`4` | Categorías, Proyectos, Revisar, Excluido | ídem |
| `e` | exportar el informe (elegir carpeta → vista previa → confirmar) | ídem |
| `q` | salir | salir |

Íconos: los simples (`► ■ ♦ ×`) se ven en el cmd de Windows 10; `--iconos emoji` y `--iconos nerd`
(con una [Nerd Font](https://www.nerdfonts.com)) quedan mejor en Windows Terminal o en Linux.

## El informe

- **Markdown:** resumen por categoría con fechas, los 20 más pesados de cada una, proyectos,
  carpetas para revisar, lo excluido y lo que no se pudo leer.
- **CSV:** una fila por archivo (`ruta;categoria;zona;proyecto;carpeta_revisar;bytes;tamano;modificado`),
  separado por `;` y en UTF-8 con BOM, para que el Excel en español lo abra bien con doble clic.
- Antes de escribir siempre muestra qué archivos va a crear y pide confirmación. Si ya existen,
  avisa que se van a sobrescribir. Si alguno aparece entre la vista previa y la escritura, no lo pisa.

## Qué hace

**Categorías:** Fotos, Videos, Música, Grabaciones de voz, Documentos, Proyectos/código,
Comprimidos, Instaladores, Otros.

- Se clasifica por extensión, sin importar mayúsculas.
- El audio (`.m4a`, `.wav`, `.mp3`…) es Música, salvo que la ruta indique que es una grabación
  ("Grabaciones", "Recordings", "Voice Notes", `PTT-` de WhatsApp). `.opus` y `.amr` son siempre notas de voz.
- Una carpeta con `.git`, `package.json`, `Cargo.toml`, `go.mod`, `*.sln`, Unity, Godot, etc.
  es un **proyecto**, y todo lo que tiene adentro cuenta como parte de él.
  No se marcan como proyecto la raíz del disco, la carpeta personal ni Documentos, Escritorio, etc.,
  aunque tengan un `package.json` suelto.

**Se excluye por ser del sistema:**

| Windows | Linux |
|---|---|
| `Windows`, `Program Files`, `Program Files (x86)`, `ProgramData`, `pagefile.sys`, `hiberfil.sys`, `swapfile.sys` (en la raíz del disco o del escaneo) | `/usr`, `/proc`, `/sys`, `/dev`, `/run`, `/boot`, `/var/lib` |
| `$Recycle.Bin` y `System Volume Information` (en cualquier nivel) | |

Una carpeta `Windows` dentro de Documentos **no** se excluye. Los enlaces simbólicos y las junctions
de Windows (`Mis documentos`, `Application Data`) no se siguen, para no contar dos veces.

**Revisar:** `AppData` y las carpetas con punto de la carpeta personal (`.config`, `.local`, `.vscode`…)
no son sistema: ahí hay partidas guardadas, perfiles de navegador, WhatsApp de escritorio.
Se listan aparte, agrupadas por programa (`AppData\Roaming\WhatsApp`), para revisarlas a mano.

## Compilar y testear

Hace falta [Rust](https://rustup.rs) (en Windows, también las herramientas de C++ de Visual Studio).

```sh
cargo test               # clasificación, exclusiones, escaneo, árbol, navegación, informe y pantalla
cargo build --release    # ejecutable en target/release/analizador(.exe)
cargo run --release --example captura -- <ruta> [ancho] [alto]   # la interfaz como texto
```

Los tests de integración crean la carpeta de demo (con fotos, documentos, proyectos, AppData y carpetas
de sistema falsas) y verifican cada categoría, cada exclusión, la agrupación de "revisar",
que el resultado con 1 hilo sea igual que en paralelo, y que **el escaneo no modifique ningún archivo**.

La interfaz está separada en un estado (`app.rs`, qué hace cada tecla) y un dibujo (`ui.rs`). Así la
navegación se testea entera sin terminal, y la pantalla se dibuja sobre el `TestBackend` de ratatui
para verificar lo que se ve, incluso en terminales chicas y con emoji. La exportación se prueba con
nombres raros (`;`, comillas, acentos) y verificando que nunca pise un archivo sin permiso.

## Por qué Rust

- Genera un ejecutable único, sin runtime, que se copia en un pendrive y anda.
- Paralelismo sin miedo (rayon) para recorrer carpetas en varios hilos.
- [ratatui](https://ratatui.rs) es la mejor opción para la interfaz de dos paneles en la terminal.
- Para leer el MFT de NTFS directamente (como WizTree) hay acceso a la API de Windows sin
  intermediarios ni garbage collector.

Go con Bubble Tea habría sido más simple de escribir, pero Rust rinde más en las fases de velocidad.

## Evolución

| Fase | Estado | Qué trae |
|---|---|---|
| 1. Escaneo y clasificación en la terminal | ✅ | Categorías, exclusiones, "revisar", proyectos, tiempo y archivos/s, carpeta de demo, tests |
| 2. Interfaz de dos paneles (ratatui) + informe Markdown/CSV | ✅ | Árbol navegable con tamaños, clasificación de la carpeta seleccionada, saltos de un panel al otro, exportación con vista previa |
| 3. Duplicados y "probablemente borrable" | ⏳ | |
| 4. Acciones: copiar/mover/`_para_borrar`, con confirmación y dry-run | ⏳ | |
| 5. Lectura del MFT en Windows | ⏳ | |
| 6. Build optimizado y ejecutables para Windows y Linux | ⏳ | |

### Fase 1: primeras mediciones

Carpeta de usuario en Windows 10, SSD NVMe, caché caliente:

| Archivos | Carpetas | Hilos | Tiempo | Archivos/s |
|---:|---:|---:|---:|---:|
| 1.161.754 | 175.666 | todos (por defecto) | 8,8 s | 131.600 |
| 1.161.754 | 175.666 | 1 | 13,8 s | 84.000 |

En Windows se usa `read_dir`, que trae tamaño y fecha en la misma llamada que lista la carpeta
(`FindNextFile`), sin abrir cada archivo.

### Fase 2: la interfaz con un disco real

Con la misma carpeta (1,16 millones de archivos): armar el árbol tarda ~1 s, cada tecla ~5 ms
y cada pantalla ~1,5 ms.
