use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};

use analizador_de_disco::escaner::{Opciones, escanear};
use analizador_de_disco::{demo, formato, informe};

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
    /// Crea una carpeta de prueba con archivos de cada tipo y la escanea
    Demo {
        /// Dónde crearla (por defecto, en la carpeta temporal del sistema)
        #[arg(long)]
        carpeta: Option<PathBuf>,
        #[command(flatten)]
        comunes: Comunes,
    },
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
        Comando::Demo { carpeta, comunes } => {
            let carpeta = carpeta.unwrap_or_else(|| std::env::temp_dir().join("analizador-demo"));
            demo::crear(&carpeta).and_then(|()| {
                eprintln!("Carpeta de prueba creada en {}", carpeta.display());
                ejecutar(&carpeta, &comunes)
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
    let opciones = Opciones {
        hilos: comunes.hilos,
    };
    let progreso = AtomicU64::new(0);
    let terminado = AtomicBool::new(false);

    let escaneo = thread::scope(|s| {
        if io::stderr().is_terminal() {
            s.spawn(|| mostrar_progreso(&progreso, &terminado));
        }
        let r = escanear(ruta, &opciones, &progreso);
        terminado.store(true, Ordering::Relaxed);
        r
    })
    .map_err(|err| io::Error::new(err.kind(), format!("{}: {err}", ruta.display())))?;

    let mut out = io::stdout().lock();
    informe::imprimir(&escaneo, comunes.top, &mut out)?;
    out.flush()
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
