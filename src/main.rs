use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};

use analizador_de_disco::escaner::{Escaneo, Opciones, escanear};
use analizador_de_disco::iconos::Iconos;
use analizador_de_disco::{demo, exportar, formato, informe, tui};

/// Escanea un disco y separa lo que creó una persona de lo que es sistema o programas.
/// Solo lectura: nunca borra ni modifica nada.
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
        /// Hilos de lectura (por defecto, uno por núcleo)
        #[arg(long)]
        hilos: Option<usize>,
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
            hilos,
        } => generar_informe(&ruta, &salida, si, hilos),
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

fn generar_informe(ruta: &Path, salida: &Path, si: bool, hilos: Option<usize>) -> io::Result<()> {
    let escaneo = escanear_con_progreso(ruta, &Opciones { hilos })?;
    let plan = exportar::planificar(salida, &escaneo, chrono::Local::now());
    let existentes = plan.existentes();

    println!("Vista previa. Se van a crear:");
    println!("  {}  (resumen)", plan.markdown.display());
    println!(
        "  {}  ({} filas)",
        plan.csv.display(),
        formato::miles(plan.filas_csv as u64)
    );
    for p in &existentes {
        println!("  ¡Ojo! {} ya existe y se va a sobrescribir.", p.display());
    }

    if !si {
        if !io::stdin().is_terminal() {
            return Err(io::Error::other(
                "no hay a quién preguntar; usá --si para confirmar sin preguntar",
            ));
        }
        print!("¿Confirmás? [s/N] ");
        io::stdout().flush()?;
        let mut respuesta = String::new();
        io::stdin().read_line(&mut respuesta)?;
        if !matches!(respuesta.trim().to_lowercase().as_str(), "s" | "si" | "sí") {
            println!("Cancelado, no se escribió nada.");
            return Ok(());
        }
    }
    let sobrescribir = !existentes.is_empty();
    exportar::escribir(&plan, &escaneo, sobrescribir)?;
    println!("Listo.");
    Ok(())
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
            formato::miles(progreso.load(Ordering::Relaxed))
        );
        let _ = err.flush();
        thread::sleep(Duration::from_millis(200));
    }
    let _ = write!(err, "\r{:60}\r", "");
}
