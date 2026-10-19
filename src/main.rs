use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};

use analizador_de_disco::acciones::{
    self, Elemento, Modo, OpcionesOrdenar, Plan, ProgresoAccion, Tipo,
};
use analizador_de_disco::borrable::{self, Motivo};
use analizador_de_disco::categoria::Categoria;
use analizador_de_disco::duplicados::{self, Duplicados, Progreso};
use analizador_de_disco::escaner::{Escaneo, Opciones, escanear};
use analizador_de_disco::formato::{duracion, miles, relativa, tamano};
use analizador_de_disco::iconos::Iconos;
use analizador_de_disco::{demo, exportar, informe, tui};

/// Escanea un disco y separa lo que creó una persona de lo que es sistema o programas.
/// El escaneo es de solo lectura. Las acciones (ordenar, para-borrar, deshacer) muestran
/// antes qué van a hacer y piden confirmación; nunca borran nada.
#[derive(Parser)]
#[command(name = "analizador", version)]
struct Cli {
    #[command(subcommand)]
    comando: Comando,
}

#[derive(Subcommand)]
enum Comando {
    /// Escanea una carpeta o un disco entero (C:\, /, E:\, /media/pendrive…)
    Escanear {
        ruta: PathBuf,
        #[command(flatten)]
        comunes: Comunes,
    },
    /// Abre la interfaz de dos paneles: el disco a la izquierda, lo clasificado a la derecha
    Ver {
        ruta: PathBuf,
        #[command(flatten)]
        interfaz: Interfaz,
    },
    /// Escanea y guarda un informe en Markdown y CSV (muestra antes qué va a escribir)
    Informe {
        ruta: PathBuf,
        /// Carpeta donde guardar el informe
        #[arg(long, default_value = ".")]
        salida: PathBuf,
        /// No preguntar antes de escribir (para scripts)
        #[arg(long)]
        si: bool,
        /// Buscar también duplicados (lee el contenido de los archivos: tarda más)
        #[arg(long)]
        duplicados: bool,
        /// Hilos de lectura (por defecto, uno por núcleo)
        #[arg(long)]
        hilos: Option<usize>,
    },
    /// Busca archivos duplicados (lee el contenido) y muestra cuánto se libera
    Duplicados {
        ruta: PathBuf,
        /// Cuántos grupos mostrar (0 = todos)
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Hilos de lectura (por defecto, uno por núcleo). En un HDD probá con pocos.
        #[arg(long)]
        hilos: Option<usize>,
    },
    /// Ordena lo que conservás en un destino: Fotos/2019/…, Documentos/…, Proyectos/…
    /// (mueve por defecto; muestra antes qué va a hacer)
    Ordenar {
        ruta: PathBuf,
        /// Carpeta de destino (por ejemplo, un disco externo)
        #[arg(long)]
        destino: PathBuf,
        /// Qué categorías (por defecto, todas menos instaladores)
        #[arg(long, value_enum, value_delimiter = ',')]
        categorias: Vec<CategoriaArg>,
        /// Copiar en vez de mover (los originales quedan donde están)
        #[arg(long)]
        copiar: bool,
        /// Ordenar también los instaladores
        #[arg(long)]
        incluir_instaladores: bool,
        /// No buscar duplicados antes (más rápido, pero se ordenan todas las copias)
        #[arg(long)]
        sin_duplicados: bool,
        /// No preguntar antes de hacerlo (para scripts)
        #[arg(long)]
        si: bool,
        #[arg(long)]
        hilos: Option<usize>,
    },
    /// Mueve lo probablemente borrable a <ruta>/_para_borrar (que después borrás vos)
    ParaBorrar {
        ruta: PathBuf,
        /// Qué motivos (por defecto: temporal, cache e instalador)
        #[arg(long, value_enum, value_delimiter = ',')]
        motivos: Vec<MotivoArg>,
        /// Mandar también las copias duplicadas (busca duplicados antes)
        #[arg(long)]
        duplicados: bool,
        /// No preguntar antes de hacerlo (para scripts)
        #[arg(long)]
        si: bool,
        #[arg(long)]
        hilos: Option<usize>,
    },
    /// Devuelve todo a su lugar, según el registro de _para_borrar o de un ordenar
    Deshacer {
        /// La carpeta escaneada, su _para_borrar, o un ordenar-….csv de _analizador
        ruta: PathBuf,
        /// No preguntar antes de hacerlo (para scripts)
        #[arg(long)]
        si: bool,
    },
    /// Crea una carpeta de prueba con archivos de cada tipo y la escanea
    Demo {
        /// Dónde crearla (por defecto, en la carpeta temporal del sistema)
        #[arg(long)]
        carpeta: Option<PathBuf>,
        /// Abrirla en la interfaz de dos paneles en vez de imprimir el resumen
        #[arg(long)]
        ver: bool,
        #[command(flatten)]
        comunes: Comunes,
        #[arg(long, value_enum, default_value_t)]
        iconos: Iconos,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum CategoriaArg {
    Fotos,
    Videos,
    #[value(alias = "música")]
    Musica,
    Grabaciones,
    Documentos,
    Proyectos,
    Comprimidos,
    Instaladores,
    Otros,
}

impl From<CategoriaArg> for Categoria {
    fn from(c: CategoriaArg) -> Self {
        match c {
            CategoriaArg::Fotos => Categoria::Fotos,
            CategoriaArg::Videos => Categoria::Videos,
            CategoriaArg::Musica => Categoria::Musica,
            CategoriaArg::Grabaciones => Categoria::Grabaciones,
            CategoriaArg::Documentos => Categoria::Documentos,
            CategoriaArg::Proyectos => Categoria::Proyectos,
            CategoriaArg::Comprimidos => Categoria::Comprimidos,
            CategoriaArg::Instaladores => Categoria::Instaladores,
            CategoriaArg::Otros => Categoria::Otros,
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum MotivoArg {
    Temporal,
    #[value(alias = "caché")]
    Cache,
    Instalador,
}

impl From<MotivoArg> for Motivo {
    fn from(m: MotivoArg) -> Self {
        match m {
            MotivoArg::Temporal => Motivo::Temporal,
            MotivoArg::Cache => Motivo::Cache,
            MotivoArg::Instalador => Motivo::Instalador,
        }
    }
}

#[derive(clap::Args)]
struct Interfaz {
    /// Íconos: simples (andan en cualquier consola), emoji o nerd (requiere una Nerd Font)
    #[arg(long, value_enum, default_value_t)]
    iconos: Iconos,
    /// Hilos de lectura (por defecto, uno por núcleo). En un HDD probá con pocos.
    #[arg(long)]
    hilos: Option<usize>,
}

#[derive(clap::Args)]
struct Comunes {
    /// Cuántos archivos más pesados mostrar por categoría (0 = ninguno)
    #[arg(long, default_value_t = 3)]
    top: usize,
    /// Hilos de lectura (por defecto, uno por núcleo). En un HDD probá con pocos.
    #[arg(long)]
    hilos: Option<usize>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let resultado = match cli.comando {
        Comando::Escanear { ruta, comunes } => ejecutar(&ruta, &comunes),
        Comando::Ver { ruta, interfaz } => tui::ejecutar(
            &ruta,
            Opciones {
                hilos: interfaz.hilos,
            },
            interfaz.iconos,
        ),
        Comando::Informe {
            ruta,
            salida,
            si,
            duplicados,
            hilos,
        } => generar_informe(&ruta, &salida, si, duplicados, hilos),
        Comando::Duplicados { ruta, top, hilos } => mostrar_duplicados(&ruta, top, hilos),
        Comando::Ordenar {
            ruta,
            destino,
            categorias,
            copiar,
            incluir_instaladores,
            sin_duplicados,
            si,
            hilos,
        } => ordenar(
            &ruta,
            &destino,
            &categorias,
            OpcionesOrdenar {
                modo: Some(if copiar { Modo::Copiar } else { Modo::Mover }),
                incluir_instaladores,
            },
            !sin_duplicados,
            si,
            hilos,
        ),
        Comando::ParaBorrar {
            ruta,
            motivos,
            duplicados,
            si,
            hilos,
        } => para_borrar(&ruta, &motivos, duplicados, si, hilos),
        Comando::Deshacer { ruta, si } => deshacer(&ruta, si),
        Comando::Demo {
            carpeta,
            ver,
            comunes,
            iconos,
        } => {
            let carpeta = carpeta.unwrap_or_else(|| std::env::temp_dir().join("analizador-demo"));
            demo::crear(&carpeta).and_then(|()| {
                if ver {
                    let opciones = Opciones {
                        hilos: comunes.hilos,
                    };
                    tui::ejecutar(&carpeta, opciones, iconos)
                } else {
                    eprintln!("Carpeta de prueba creada en {}", carpeta.display());
                    ejecutar(&carpeta, &comunes)
                }
            })
        }
    };
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn ejecutar(ruta: &Path, comunes: &Comunes) -> io::Result<()> {
    let escaneo = escanear_con_progreso(
        ruta,
        &Opciones {
            hilos: comunes.hilos,
        },
    )?;
    let mut out = io::stdout().lock();
    informe::imprimir(&escaneo, comunes.top, &mut out)?;
    out.flush()
}

fn generar_informe(
    ruta: &Path,
    salida: &Path,
    si: bool,
    con_duplicados: bool,
    hilos: Option<usize>,
) -> io::Result<()> {
    let escaneo = escanear_con_progreso(ruta, &Opciones { hilos })?;
    let dups = if con_duplicados {
        Some(duplicados_con_progreso(&escaneo, hilos)?)
    } else {
        None
    };
    let plan = exportar::planificar(salida, &escaneo, chrono::Local::now());
    let existentes = plan.existentes();

    println!("Vista previa. Se van a crear:");
    println!("  {}  (resumen)", plan.markdown.display());
    println!(
        "  {}  ({} filas)",
        plan.csv.display(),
        miles(plan.filas_csv as u64)
    );
    for p in &existentes {
        println!("  ¡Ojo! {} ya existe y se va a sobrescribir.", p.display());
    }

    if !confirmar(si)? {
        println!("Cancelado, no se escribió nada.");
        return Ok(());
    }
    let sobrescribir = !existentes.is_empty();
    exportar::escribir(&plan, &escaneo, dups.as_ref(), sobrescribir)?;
    println!("Listo.");
    Ok(())
}

/// Pregunta `¿Confirmás? [s/N]`. Sin terminal, solo sigue con `--si`.
fn confirmar(si: bool) -> io::Result<bool> {
    if si {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        return Err(io::Error::other(
            "no hay a quién preguntar; usá --si para confirmar sin preguntar",
        ));
    }
    print!("¿Confirmás? [s/N] ");
    io::stdout().flush()?;
    let mut respuesta = String::new();
    io::stdin().read_line(&mut respuesta)?;
    Ok(matches!(
        respuesta.trim().to_lowercase().as_str(),
        "s" | "si" | "sí"
    ))
}

fn ordenar(
    ruta: &Path,
    destino: &Path,
    categorias: &[CategoriaArg],
    opciones: OpcionesOrdenar,
    con_duplicados: bool,
    si: bool,
    hilos: Option<usize>,
) -> io::Result<()> {
    let escaneo = escanear_con_progreso(ruta, &Opciones { hilos })?;
    let dups = if con_duplicados {
        Some(duplicados_con_progreso(&escaneo, hilos)?)
    } else {
        None
    };
    let elegidas: Vec<Categoria> = categorias.iter().map(|&c| c.into()).collect();
    let elementos: Vec<Elemento> = (0..escaneo.archivos.len() as u32)
        .filter(|&i| {
            let a = &escaneo.archivos[i as usize];
            a.revisar.is_none() && (elegidas.is_empty() || elegidas.contains(&a.categoria))
        })
        .map(|i| Elemento::de(&escaneo, i))
        .collect();
    let mut opciones = opciones;
    opciones.incluir_instaladores |= elegidas.contains(&Categoria::Instaladores);
    eprintln!("Armando el plan (leyendo la fecha EXIF de las fotos)…");
    let plan = acciones::planificar_ordenar(
        &elementos,
        &escaneo.raiz,
        destino,
        opciones,
        dups.as_ref(),
        chrono::Local::now(),
    );
    hacer(&plan, &escaneo.raiz, si)
}

fn para_borrar(
    ruta: &Path,
    motivos: &[MotivoArg],
    con_duplicados: bool,
    si: bool,
    hilos: Option<usize>,
) -> io::Result<()> {
    let escaneo = escanear_con_progreso(ruta, &Opciones { hilos })?;
    let dups = if con_duplicados {
        Some(duplicados_con_progreso(&escaneo, hilos)?)
    } else {
        None
    };
    let mut elegidos: Vec<Motivo> = motivos.iter().map(|&m| m.into()).collect();
    if elegidos.is_empty() {
        elegidos = vec![Motivo::Temporal, Motivo::Cache, Motivo::Instalador];
    }
    if con_duplicados {
        elegidos.push(Motivo::Duplicado);
    }
    let elementos: Vec<Elemento> = borrable::por_archivo(&escaneo, dups.as_ref())
        .iter()
        .enumerate()
        .filter(|(_, m)| m.as_ref().is_some_and(|(m, _)| elegidos.contains(m)))
        .map(|(i, _)| Elemento::de(&escaneo, i as u32))
        .collect();
    let plan = acciones::planificar_para_borrar(&elementos, &escaneo.raiz);
    hacer(&plan, &escaneo.raiz, si)
}

fn deshacer(ruta: &Path, si: bool) -> io::Result<()> {
    let plan = acciones::planificar_deshacer(ruta)?;
    let raiz = plan.destino.parent().unwrap_or(ruta).to_path_buf();
    hacer(&plan, &raiz, si)
}

/// Muestra la vista previa, pide confirmación y ejecuta mostrando el avance.
fn hacer(plan: &Plan, raiz: &Path, si: bool) -> io::Result<()> {
    mostrar_plan(plan, raiz);
    if !plan.problemas.is_empty() {
        return Err(io::Error::other("no se puede seguir: no se hizo nada"));
    }
    if plan.operaciones.is_empty() {
        println!("No hay nada para hacer.");
        return Ok(());
    }
    if !confirmar(si)? {
        println!("Cancelado, no se tocó nada.");
        return Ok(());
    }

    let progreso = ProgresoAccion::default();
    let terminado = AtomicBool::new(false);
    let total = plan.operaciones.len() as u64;
    let r = thread::scope(|s| {
        if io::stderr().is_terminal() {
            s.spawn(|| {
                let mut err = io::stderr();
                while !terminado.load(Ordering::Relaxed) {
                    let _ = write!(
                        err,
                        "\r  {} de {} archivos · {} de {}   ",
                        miles(progreso.hechos.load(Ordering::Relaxed)),
                        miles(total),
                        tamano(progreso.bytes.load(Ordering::Relaxed)),
                        tamano(plan.bytes)
                    );
                    let _ = err.flush();
                    thread::sleep(Duration::from_millis(200));
                }
                let _ = write!(err, "\r{:70}\r", "");
            });
        }
        let r = acciones::ejecutar(plan, &progreso);
        terminado.store(true, Ordering::Relaxed);
        r
    })?;

    println!(
        "\nListo: {} archivos ({}) en {}.",
        miles(r.hechos as u64),
        tamano(r.bytes),
        duracion(r.duracion)
    );
    if r.renombrados > 0 {
        println!(
            "  {} quedaron con \" (2)\" porque apareció otro con el mismo nombre.",
            miles(r.renombrados as u64)
        );
    }
    if !r.fallidos.is_empty() {
        println!("  No se pudieron hacer {}:", miles(r.fallidos.len() as u64));
        for (op, err) in r.fallidos.iter().take(10) {
            println!("    {}: {err}", op.origen.display());
        }
        if r.fallidos.len() > 10 {
            println!(
                "    … y {} más (están en el registro)",
                r.fallidos.len() - 10
            );
        }
    }
    match plan.tipo {
        Tipo::Deshacer => {}
        Tipo::Ordenar(Modo::Copiar) => println!("  Registro: {}", plan.registro.display()),
        _ => {
            println!("  Registro: {}", plan.registro.display());
            println!(
                "  Para deshacer: analizador deshacer \"{}\"",
                plan.registro.display()
            );
            println!("  Las carpetas que quedaron vacías no se borran.");
        }
    }
    Ok(())
}

fn mostrar_plan(plan: &Plan, raiz: &Path) {
    println!("\nVista previa (todavía no se hizo nada):");
    for (_, linea) in acciones::describir(plan, raiz, 6) {
        println!("  {linea}");
    }
}

fn mostrar_duplicados(ruta: &Path, top: usize, hilos: Option<usize>) -> io::Result<()> {
    let escaneo = escanear_con_progreso(ruta, &Opciones { hilos })?;
    let d = duplicados_con_progreso(&escaneo, hilos)?;
    let mut out = io::stdout().lock();
    writeln!(out, "\nDuplicados en {}", escaneo.raiz.display())?;
    let cantidad = if top == 0 { d.grupos.len() } else { top };
    for g in d.grupos.iter().take(cantidad) {
        writeln!(
            out,
            "\n  {} liberables · {} copias de {}",
            tamano(g.liberable()),
            g.archivos.len(),
            tamano(g.tamano)
        )?;
        for (i, &a) in g.archivos.iter().enumerate() {
            let marca = if i == 0 { "conservar" } else { "copia    " };
            let ruta = &escaneo.archivos[a as usize].ruta;
            writeln!(out, "    {marca}  {}", relativa(&escaneo.raiz, ruta))?;
        }
    }
    if d.grupos.len() > cantidad {
        writeln!(out, "\n  … y {} grupos más", d.grupos.len() - cantidad)?;
    }
    match d.errores.len() {
        0 => {}
        1 => writeln!(out, "\n  No se pudo leer 1 archivo.")?,
        n => writeln!(out, "\n  No se pudieron leer {} archivos.", miles(n as u64))?,
    }
    writeln!(
        out,
        "\n{} grupos · {} copias de más · se liberan {}",
        miles(d.grupos.len() as u64),
        miles(d.copias() as u64),
        tamano(d.liberable())
    )?;
    writeln!(
        out,
        "Tiempo: {} · {} candidatos · {} leídos · {:.0} MB/s",
        duracion(d.duracion),
        miles(d.candidatos as u64),
        tamano(d.bytes_leidos),
        d.mb_por_segundo()
    )?;
    out.flush()
}

fn duplicados_con_progreso(e: &Escaneo, hilos: Option<usize>) -> io::Result<Duplicados> {
    let candidatos = duplicados::candidatos(e);
    eprintln!(
        "Buscando duplicados entre {} archivos que comparten tamaño ({})…",
        miles(candidatos.len() as u64),
        tamano(duplicados::bytes_candidatos(&candidatos))
    );
    let progreso = Progreso::default();
    let terminado = AtomicBool::new(false);
    thread::scope(|s| {
        if io::stderr().is_terminal() {
            s.spawn(|| {
                let inicio = Instant::now();
                let mut err = io::stderr();
                while !terminado.load(Ordering::Relaxed) {
                    let hechos = progreso.bytes_hechos.load(Ordering::Relaxed);
                    let total = progreso.bytes_total.load(Ordering::Relaxed);
                    let etapa = progreso.etapa.load(Ordering::Relaxed);
                    let _ = write!(
                        err,
                        "\r  etapa {etapa}/2 · {} de {} · {}   ",
                        tamano(hechos),
                        tamano(total),
                        duracion(inicio.elapsed())
                    );
                    let _ = err.flush();
                    thread::sleep(Duration::from_millis(200));
                }
                let _ = write!(err, "\r{:70}\r", "");
            });
        }
        let r = duplicados::buscar(candidatos, &progreso, hilos);
        terminado.store(true, Ordering::Relaxed);
        r
    })
}

fn escanear_con_progreso(ruta: &Path, opciones: &Opciones) -> io::Result<Escaneo> {
    let progreso = AtomicU64::new(0);
    let terminado = AtomicBool::new(false);

    thread::scope(|s| {
        if io::stderr().is_terminal() {
            s.spawn(|| mostrar_progreso(&progreso, &terminado));
        }
        let r = escanear(ruta, opciones, &progreso);
        terminado.store(true, Ordering::Relaxed);
        r
    })
    .map_err(|err| io::Error::new(err.kind(), format!("{}: {err}", ruta.display())))
}

fn mostrar_progreso(progreso: &AtomicU64, terminado: &AtomicBool) {
    let mut err = io::stderr();
    while !terminado.load(Ordering::Relaxed) {
        let _ = write!(
            err,
            "\r  escaneando… {} archivos",
            miles(progreso.load(Ordering::Relaxed))
        );
        let _ = err.flush();
        thread::sleep(Duration::from_millis(200));
    }
    let _ = write!(err, "\r{:60}\r", "");
}
