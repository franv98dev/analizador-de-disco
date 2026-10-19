---
fecha: 2026-10-19
para: Claude Code (💻)
proyecto: Analizador de disco
cómo usarlo: creá una carpeta para el proyecto (ej. D:\Proyectos\analizador), copiá este archivo adentro como CLAUDE.md, abrí una terminal ahí, corré `claude` y decile: "Leé CLAUDE.md y arrancá por la fase 1".
---

# Analizador de disco — pedido para Claude Code

## Para qué es
Escanear el disco de una PC (la de mi papá, mi notebook vieja, la mía) y mostrarme **todo lo que creó una persona** (fotos, videos, documentos, grabaciones, proyectos), separado de lo que es sistema o programas. Con eso decido **qué conservar y qué borrar** antes de convertir una de esas máquinas en un **servidor Linux** para mi nube local.

## Cómo me lo imagino (de mis notas de voz)
- Como el analizador de disco del **PC Manager de Microsoft**, pero **más liviano** y con **vibra de consola moderna**.
- **Dos paneles**, uno al lado del otro, que parezcan una consola pero con íconos e información: a un lado el disco escaneado y al otro el resultado clasificado.
- Que **clasifique** lo del usuario por tipo y, si se lo pido, lo **ordene en carpetas** en un destino que yo elija.
- **Compilado y optimizado** para la máquina donde corre, y **rápido**: quiero saber cuánto tarda en revisar un disco.

## Requisitos
1. **Un solo ejecutable, sin instalar nada**, para **Windows x64** (la PC de papá y la notebook vieja probablemente tengan Windows) y **Linux x64**. Sugerencia: **Rust + ratatui** para la interfaz de terminal; Go + Bubble Tea también sirve. Elegí y justificá.
2. **Escaneo de solo lectura.** Nunca borra nada por su cuenta.
3. **Excluir lo que es del sistema:**
   - Windows: `Windows`, `Program Files`, `ProgramData`, `$Recycle.Bin`, `System Volume Information`, `pagefile.sys` y `hiberfil.sys`.
   - Linux: `/usr`, `/proc`, `/sys`, `/dev`, `/run`, `/boot`, `/var/lib`.
   - **Ojo con `AppData`:** ahí viven cosas valiosas (partidas guardadas, perfiles de navegador, WhatsApp de escritorio). Marcalas como "revisar", no como sistema.
4. **Categorías:** Fotos, Videos, Música, Grabaciones de voz, Documentos (PDF, Office, texto), Proyectos/código (carpetas con `.git`, `package.json`, etc.), Comprimidos, Instaladores (`.exe`, `.msi`, `.iso`), Otros.
5. **Ayuda para decidir:**
   - **Duplicados:** primero por tamaño, después por hash. Que muestre cuánto espacio libero borrando las copias.
   - **Los más pesados:** los archivos y carpetas que más ocupan.
   - **Probablemente borrable:** instaladores, temporales, cachés, duplicados.
   - **Por antigüedad:** cuándo se modificó cada cosa por última vez.
6. **Acciones, siempre con confirmación y con vista previa antes ("dry-run"):**
   - **Exportar un informe** (Markdown y CSV) con lo encontrado, para revisarlo tranquilo.
   - **Copiar o mover lo que conservo** a un destino ordenado (`Fotos/2019/…`, `Documentos/…`).
   - **Borrar:** nada de borrar directo. Mover a una carpeta `_para_borrar` que después borro yo.
7. **Medir:** mostrar archivos por segundo y tiempo total. Quiero comparar HDD vs SSD.
8. **Discos externos y pendrives** también. El **celular** queda para una fase posterior (en Android, con `adb` o corriendo dentro de Termux).

## Velocidad: qué esperar
- Recorrer carpetas (lo que hacen casi todos los programas) tarda segundos en un SSD y **varios minutos en un HDD** con millones de archivos. Depende más de la *cantidad* de archivos que del tamaño.
- Programas como **WizTree** leen directamente el **MFT de NTFS**, el índice del disco en Windows. Así escanean un disco entero en segundos, pero necesitan permisos de administrador.
- **Propuesta:** fase 1 con recorrido en paralelo, que es simple y anda en todos lados. Si queda lento en los HDD, fase 3 con lectura del MFT en Windows.

## Fases
1. **Escaneo y clasificación en la terminal** (sin interfaz todavía): lista por categoría, tamaños, tiempo. Probarlo en una carpeta de prueba creada por el propio programa.
2. **Interfaz de dos paneles** con ratatui, más el informe en Markdown/CSV.
3. **Duplicados** y "probablemente borrable".
4. **Acciones** (copiar/mover/`_para_borrar`) con confirmación.
5. Velocidad: MFT en Windows, si hace falta.
6. Build optimizado (`--release`, `target-cpu=native` para cada máquina) y ejecutables para Windows y Linux.

## Cómo quiero trabajar
- **Repositorio Git desde el día 1** y **subido a mi GitHub (franv98dev)**, con un commit por avance y un README que muestre la evolución. Me sirve como proyecto de portfolio.
- **Tests automatizados** de clasificación y exclusiones: soy QA, que se note.
- Antes de cada fase, contame el plan en pocas líneas. Al terminarla, cómo probarla.
