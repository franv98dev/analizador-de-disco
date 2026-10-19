//! Bucle de la interfaz: escanea en otro hilo mostrando el avance y después atiende el teclado.

use std::io::{self, IsTerminal};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::app::App;
use crate::demo;
use crate::escaner::{Escaneo, Opciones, escanear};
use crate::iconos::Iconos;
use crate::inicio::{Eleccion, Inicio, carpeta_personal};
use crate::ui;
use crate::unidades;

pub fn ejecutar(ruta: &Path, opciones: Opciones, iconos: Iconos) -> io::Result<()> {
    // Antes de tomar la terminal, para que el error se lea normal.
    if !ruta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} no existe o no es una carpeta", ruta.display()),
        ));
    }
    if !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "la interfaz necesita una terminal; para guardar el resultado usá `escanear` o `informe`",
        ));
    }
    let mut terminal = ratatui::try_init().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("no se pudo abrir la interfaz en esta terminal: {err}"),
        )
    })?;
    let resultado = correr(&mut terminal, ruta, opciones, iconos);
    ratatui::restore();
    resultado
}

/// Sin argumentos (doble click): primero la pantalla de inicio para elegir qué revisar, y
/// al salir de un escaneo se vuelve a ella en vez de cerrar la ventana.
pub fn inicio(opciones: Opciones, iconos: Iconos) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "la interfaz necesita una terminal; probá `analizador --help`",
        ));
    }
    let mut terminal = ratatui::try_init().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("no se pudo abrir la interfaz en esta terminal: {err}"),
        )
    })?;
    let resultado = pantalla_de_inicio(&mut terminal, opciones, iconos);
    ratatui::restore();
    resultado
}

fn pantalla_de_inicio(
    terminal: &mut DefaultTerminal,
    opciones: Opciones,
    iconos: Iconos,
) -> io::Result<()> {
    let nuevo = || Inicio::new(unidades::detectar(), carpeta_personal());
    let mut ini = nuevo();
    loop {
        terminal.draw(|f| ui::dibujar_inicio(f, &mut ini))?;
        let Event::Key(k) = event::read()? else {
            continue;
        };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(());
        }
        let ruta = match ini.tecla(k.code) {
            None => continue,
            Some(Eleccion::Salir) => return Ok(()),
            Some(Eleccion::Actualizar) => {
                ini = nuevo();
                continue;
            }
            Some(Eleccion::Abrir(ruta)) => ruta,
            Some(Eleccion::Demo) => {
                let carpeta = std::env::temp_dir().join("analizador-demo");
                if let Err(err) = demo::crear(&carpeta) {
                    ini.mensaje = Some(format!("No se pudo crear la demo: {err}"));
                    continue;
                }
                carpeta
            }
        };
        if !ruta.is_dir() {
            ini.mensaje = Some(format!("{} no existe o no es una carpeta", ruta.display()));
            continue;
        }
        if let Err(err) = correr(terminal, &ruta, opciones.clone(), iconos) {
            ini.mensaje = Some(format!("{}: {err}", ruta.display()));
        }
    }
}

fn correr(
    terminal: &mut DefaultTerminal,
    ruta: &Path,
    opciones: Opciones,
    iconos: Iconos,
) -> io::Result<()> {
    // Después de mover cosas lo que se ve quedó viejo: se vuelve a escanear y se sigue.
    while let Some(escaneo) = escanear_mostrando(terminal, ruta, &opciones)? {
        // Con millones de archivos, armar el árbol tarda alrededor de un segundo.
        terminal.draw(|f| ui::dibujar_clasificando(f, escaneo.archivos.len() as u64))?;
        let mut app = App::new(&escaneo, iconos);
        while !app.salir {
            app.tick();
            terminal.draw(|f| ui::dibujar(f, &mut app))?;
            // Mientras algo corre en otro hilo se redibuja solo, para mostrar el avance.
            if app.ocupado() && !event::poll(Duration::from_millis(150))? {
                continue;
            }
            // En Windows llegan también los eventos de soltar la tecla: se ignoran.
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
            {
                app.tecla(k);
            }
        }
        if !app.reescanear {
            break;
        }
    }
    Ok(())
}

/// Escanea en otro hilo mostrando el avance. `None` si se canceló.
fn escanear_mostrando(
    terminal: &mut DefaultTerminal,
    ruta: &Path,
    opciones: &Opciones,
) -> io::Result<Option<Escaneo>> {
    let progreso = Arc::new(AtomicU64::new(0));
    let (tx, rx) = mpsc::channel();
    {
        let progreso = Arc::clone(&progreso);
        let ruta = ruta.to_path_buf();
        let opciones = opciones.clone();
        // Si se cancela, el hilo queda suelto y muere al terminar el programa: solo lee.
        thread::spawn(move || {
            let _ = tx.send(escanear(&ruta, &opciones, &progreso));
        });
    }

    let inicio = Instant::now();
    loop {
        match rx.try_recv() {
            Ok(r) => return r.map(Some),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("el escaneo se interrumpió"));
            }
        }
        terminal.draw(|f| {
            ui::dibujar_progreso(f, ruta, progreso.load(Ordering::Relaxed), inicio.elapsed())
        })?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
            && (matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
                || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)))
        {
            return Ok(None);
        }
    }
}
