# Analizador de disco

Escanea el disco de una PC y separa **lo que creó una persona** (fotos, videos, documentos,
grabaciones, proyectos) de lo que es **sistema o programas**. Sirve para decidir qué conservar
y qué borrar antes de reciclar una máquina, por ejemplo para convertirla en un servidor Linux.

- Un solo ejecutable, sin instalar nada (Windows x64 y Linux x64).
- **Solo lectura**: el escaneo nunca modifica ni borra nada.
- Recorrido en paralelo y medición de velocidad (archivos/s), para comparar HDD y SSD.

## Uso

```sh
analizador escanear C:\                 # un disco entero
analizador escanear /media/pendrive     # un pendrive o disco externo
analizador escanear ~ --top 10          # los 10 más pesados de cada categoría
analizador escanear D:\ --hilos 2       # menos hilos: a veces rinde más en un HDD
analizador demo                         # crea una carpeta de prueba y la escanea
```

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
cargo test               # tests de clasificación, exclusiones y escaneo de la carpeta de demo
cargo build --release    # ejecutable en target/release/analizador(.exe)
```

Los tests de integración crean la carpeta de demo (con fotos, documentos, proyectos, AppData y carpetas
de sistema falsas) y verifican cada categoría, cada exclusión, la agrupación de "revisar",
que el resultado con 1 hilo sea igual que en paralelo, y que **el escaneo no modifique ningún archivo**.

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
| 2. Interfaz de dos paneles (ratatui) + informe Markdown/CSV | ⏳ | |
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
