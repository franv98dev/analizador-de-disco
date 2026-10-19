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
use crate::escaner::{Escaneo, Opciones, escanear};
use crate::iconos::Iconos;
use crate::ui;

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
