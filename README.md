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
analizador informe C:\ --duplicados     # el informe con duplicados (lee el contenido: tarda más)
analizador duplicados D:\ --top 50      # solo duplicados: grupos de copias y cuánto se libera
analizador ordenar C:\Users\papá --destino E:\Backup                 # mueve lo del usuario, ordenado
analizador ordenar C:\ --destino E:\Backup --categorias fotos,videos --copiar
analizador para-borrar C:\Users\papá --duplicados                    # lo borrable a _para_borrar
analizador deshacer C:\Users\papá                                    # todo de vuelta a su lugar
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
| `1`–`8` | Categorías, Pesados, Antigüedad, Borrable, Duplicados, Proyectos, Revisar, Excluido | ídem |
| `d` | buscar duplicados en segundo plano (otra vez `d`: cancelar) | ídem |
| `Espacio` | marcar o desmarcar la carpeta o el archivo | marcar la fila: una categoría, un año, una sugerencia de borrable, las copias de un grupo… |
| `c` | ordenar lo marcado (destino → vista previa → confirmar); `Tab` cambia entre mover y copiar | ídem |
| `x` | mandar lo marcado a `_para_borrar` (vista previa → confirmar) | ídem |
| `u` | desmarcar todo | ídem |
| `e` | exportar el informe (elegir carpeta → vista previa → confirmar) | ídem |
| `q` | salir | salir |

Íconos: los simples (`► ■ ♦ ×`) se ven en el cmd de Windows 10; `--iconos emoji` y `--iconos nerd`
(con una [Nerd Font](https://www.nerdfonts.com)) quedan mejor en Windows Terminal o en Linux.

## El informe

- **Markdown:** resumen por categoría con fechas, los 20 más pesados de cada una, probablemente
  borrable, duplicados (si se buscaron), antigüedad por año, proyectos, carpetas para revisar,
  lo excluido y lo que no se pudo leer.
- **CSV:** una fila por archivo
  (`ruta;categoria;zona;proyecto;carpeta_revisar;bytes;tamano;modificado;borrable;grupo_duplicado`),
  separado por `;` y en UTF-8 con BOM, para que el Excel en español lo abra bien con doble clic.
- Antes de escribir siempre muestra qué archivos va a crear y pide confirmación. Si ya existen,
  avisa que se van a sobrescribir. Si alguno aparece entre la vista previa y la escritura, no lo pisa.

## Ayuda para decidir

**Duplicados** (`d` en la interfaz, `analizador duplicados`, o `informe --duplicados`). En tres pasadas,
de la más barata a la más cara:

1. Por **tamaño exacto**: no lee nada, viene del escaneo. Los tamaños únicos se descartan.
2. Por un **hash del principio y el final** de cada archivo (128 KB como mucho).
3. Por el **hash completo** (BLAKE3), solo de los que siguen empatados.

Es lo único que lee el contenido de los archivos, por eso corre solo cuando se pide y muestra el avance.
De cada grupo sugiere **conservar el más viejo** y muestra cuánto se libera borrando el resto.
Si un archivo cambió entre el escaneo y la comparación, no se compara. No se miran archivos de menos
de 1 KB, ni los de "revisar" ni los de proyectos (`node_modules` está lleno de archivos idénticos).

**Probablemente borrable**, cada sugerencia con su motivo:

| Motivo | Qué se sugiere | Qué no |
|---|---|---|
| Temporal | `.tmp`, descargas a medias (`.crdownload`, `.part`), `~$` de Office, `Thumbs.db`, volcados `.dmp`, carpetas `Temp` | |
| Caché | carpetas `Cache`, `Code Cache`, `.cache`, `npm-cache`…; `node_modules` y `__pycache__` **dentro de un proyecto** | el `node_modules` de un programa instalado o el global de npm (son parte del programa) |
| Instalador | `.msi`, `.apk`, `.deb`…; `.exe` e `.iso` en Descargas o que se llaman setup/install | un programa portable o el `.iso` de un juego para emulador |
| Copia | las copias de un grupo de duplicados, nunca la que se conserva | |

Nada dentro de `Package Cache` (Windows lo usa para desinstalar). Las reglas miran la ruta **relativa
a lo escaneado**: si se escanea algo que vive dentro de `Temp`, no se marca todo como temporal.

**Los más pesados** de cualquier tipo, y la **antigüedad** por año de última modificación (con Enter en un
año se ven sus archivos), siempre de la carpeta seleccionada.

## Acciones: ordenar, `_para_borrar` y deshacer

**Ordenar** (`c` en la interfaz o `analizador ordenar`) **mueve** por defecto (`--copiar` o `Tab` para copiar)
lo que se conserva a un destino ordenado:

```
Fotos/2015/IMG_20150704_120000.jpg     ← el año sale del EXIF de la cámara; si no tiene, de la modificación
Videos/2015/cumple de la abuela.mp4
Grabaciones de voz/2020/Grabación 001.m4a
Documentos/Facultad/TP1.docx           ← Users/papá/Documents/Facultad/TP1.docx: mantiene su carpeta
Música/Soda Stereo/De música ligera.mp3
Proyectos/mi-web/src/index.js          ← los proyectos van enteros, con su estructura
```

No se llevan los instaladores (salvo que se pidan), los temporales, ni las copias duplicadas: de cada
grupo va una sola, la que se conserva. Por consola, `ordenar` busca duplicados antes (`--sin-duplicados`
para saltearlo).

**`_para_borrar`** (`x` o `analizador para-borrar`) mueve las cosas a `_para_borrar/<ruta original>` en la
raíz de lo escaneado. En el mismo disco es un renombre instantáneo. El programa no borra nada: esa carpeta
la borrás vos cuando estés seguro. No se vuelve a escanear ni a ofrecer.

**Deshacer** (`analizador deshacer`) lee el registro y devuelve todo a su lugar: el `manifiesto.csv` de
`_para_borrar`, o el `ordenar-….csv` que queda en `<destino>/_analizador/`.

Cómo se cuida lo que se mueve:

- **Siempre vista previa primero**: cuántos archivos y cuánto pesan, a qué carpetas van, ejemplos, qué se
  deja afuera y por qué, y si hay **espacio libre** en el destino. No se toca nada hasta confirmar.
- **Nunca se pisa un archivo**: si el nombre ya existe (o aparece entre la vista previa y la ejecución),
  se agrega ` (2)`. Sin distinguir mayúsculas, como en Windows.
- **Mover en el mismo disco es renombrar**: no se copia nada.
- **Entre discos, copiar + verificar + recién ahí borrar**: se escribe a un `.parcial`, se compara con
  BLAKE3 contra el original, se le pone el nombre final y después se borra el original. Si algo falla
  o se cancela, el original queda y no quedan `.parcial`.
- Se conserva la **fecha de modificación**. Si un archivo cambió desde el escaneo, no se mueve.
- No acepta destinos de sistema (`C:\Windows`, `/usr`…). Cada operación queda en el registro a medida
  que se hace, así sirve para deshacer aunque se corte la luz.
- Después de mover, la interfaz vuelve a escanear sola para mostrar cómo quedó.

La vista previa de ordenar una carpeta de usuario real (178.062 archivos, 62,9 GB, con duplicados:
1 min 18 s) mostró que iba a mover `.bash_history` y `.claude.json`: los archivos de configuración
sueltos en la carpeta personal ahora son "revisar" y no se ordenan.

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

**Revisar:** `AppData`, las carpetas con punto de la carpeta personal (`.config`, `.local`, `.vscode`…)
y los archivos con punto sueltos ahí (`.bash_history`, `.gitconfig`) no son sistema: ahí hay partidas guardadas, perfiles de navegador, WhatsApp de escritorio.
Se listan aparte, agrupadas por programa (`AppData\Roaming\WhatsApp`), para revisarlas a mano.

## Compilar y testear

Hace falta [Rust](https://rustup.rs) (en Windows, también las herramientas de C++ de Visual Studio).

```sh
cargo test               # clasificación, exclusiones, escaneo, árbol, navegación, informe, pantalla,
                         # duplicados, borrable, antigüedad, ordenar, _para_borrar y deshacer
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

Para los duplicados, cada archivo de la demo tiene contenido propio, y hay copias exactas y un
**falso duplicado** (mismo tamaño, mismo principio y mismo final, un byte distinto en el medio) que
solo la tercera pasada puede descartar. También se prueba que buscar duplicados no modifique nada,
que se pueda cancelar, y que un archivo que cambió después del escaneo no se dé por duplicado.
Las reglas de "borrable" tienen un test por cada caso que sí y por cada caso que no, varios sacados
de falsos positivos reales encontrados al probarlo en una carpeta de usuario.

Las acciones mueven archivos de verdad, así que sus tests apuntan sobre todo a lo que **no** tiene que
pasar: el dry-run no escribe nada; nunca se pisa un archivo, ni uno que aparezca después de la vista
previa; un archivo que cambió no se mueve; cancelar no deja `.parcial`; los destinos de sistema se
rechazan; y la ida y vuelta (`_para_borrar` → deshacer, ordenar → deshacer) deja todo **exactamente**
igual: mismas rutas, mismo contenido (BLAKE3) y mismas fechas. Mover entre discos se prueba de verdad
cuando la carpeta temporal y el proyecto están en discos distintos (acá, `C:` y `D:`). La demo trae
una foto cuyo EXIF dice 2015 aunque se modificó en 2023: tiene que ir a `Fotos/2015`.

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
| 3. Duplicados y "probablemente borrable" | ✅ | Duplicados en tres pasadas, borrable con motivo, más pesados, antigüedad, todo en la interfaz y en el informe |
| 4. Acciones: copiar/mover/`_para_borrar`, con confirmación y dry-run | ✅ | Ordenar por categoría y año (EXIF), `_para_borrar`, deshacer, copia verificada, marcar en la interfaz |
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

### Fase 3: duplicados y borrable con un disco real

Misma carpeta de usuario (1,16 millones de archivos, SSD NVMe):

| Qué | Resultado |
|---|---|
| Candidatos a duplicado (comparten tamaño con otro) | 83.806 archivos |
| Leído para compararlos | 7,1 GB en 1 min 05 s, 110 MB/s |
| Grupos de duplicados | 3.338 (5.500 copias de más) |
| Liberable borrando las copias | 3,2 GB |
| Probablemente borrable (temporales, cachés, instaladores) | 40,5 GB |

Se midió antes de reconocer los proyectos de Unreal (`.uproject`): ahora sus cachés internas
quedan afuera de la búsqueda de duplicados, así que los grupos serían algunos menos.

Probarlo con datos reales sacó a la luz falsos positivos que ahora tienen su test: el `node_modules`
global de npm y el de programas instalados, el `.iso` de un juego para emulador y `Package Cache`.
