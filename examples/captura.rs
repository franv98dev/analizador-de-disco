//! Dibuja la interfaz en memoria y la imprime como texto, para el README o para revisar
//! cómo se ve sin abrir la terminal interactiva.
//!
//! `cargo run --release --example captura -- <ruta> [ancho] [alto] [teclas]`
//! `cargo run --release --example captura -- --inicio [ancho] [alto]` (la pantalla de inicio)
//! Las teclas son letras sueltas: `j`/`k` bajar/subir, `l` Enter, `h` atrás, `t` Tab, `1`-`8` vistas;
//! lo que va entre comillas se escribe tal cual.

use std::sync::atomic::AtomicU64;
use std::time::Instant;

use analizador_de_disco::app::App;
use analizador_de_disco::escaner::{Opciones, escanear};
use analizador_de_disco::iconos::Iconos;
use analizador_de_disco::ui;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ruta = args.get(1).expect("falta la ruta");
    let ancho: u16 = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(120);
    let alto: u16 = args.get(3).and_then(|x| x.parse().ok()).unwrap_or(30);
    let teclas = args.get(4).cloned().unwrap_or_default();

    // `--inicio`: la pantalla de inicio (la del doble click) con los discos de esta máquina.
    if ruta == "--inicio" {
        let mut ini = analizador_de_disco::inicio::Inicio::new(
            analizador_de_disco::unidades::detectar(),
            analizador_de_disco::inicio::carpeta_personal(),
        );
        let mut t = Terminal::new(TestBackend::new(ancho, alto)).unwrap();
        t.draw(|f| ui::dibujar_inicio(f, &mut ini)).unwrap();
        let buf = t.backend().buffer();
        for y in 0..alto {
            let linea: String = (0..ancho).map(|x| buf[(x, y)].symbol()).collect();
            println!("{}", linea.trim_end());
        }
        return;
    }

    let e = escanear(ruta.as_ref(), &Opciones::default(), &AtomicU64::new(0)).unwrap();
    let inicio = Instant::now();
    let mut app = App::new(&e, Iconos::Simples);
    eprintln!("árbol armado en {:?}", inicio.elapsed());

    let mut literal = false;
    for c in teclas.chars() {
        // Entre comillas se escribe tal cual (para tipear una carpeta de destino).
        if c == '"' {
            literal = !literal;
            continue;
        }
        if literal {
            app.tecla(KeyEvent::from(KeyCode::Char(c)));
            continue;
        }
        let code = match c {
            'j' => KeyCode::Down,
            'k' => KeyCode::Up,
            'l' => KeyCode::Enter,
            'h' => KeyCode::Left,
            't' => KeyCode::Tab,
            otra => KeyCode::Char(otra),
        };
        let t = Instant::now();
        app.tecla(KeyEvent::from(code));
        eprintln!("tecla {c:?}: {:?}", t.elapsed());
        while app.ocupado() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.tick();
        }
    }

    let mut t = Terminal::new(TestBackend::new(ancho, alto)).unwrap();
    let inicio = Instant::now();
    t.draw(|f| ui::dibujar(f, &mut app)).unwrap();
    eprintln!("dibujado en {:?}", inicio.elapsed());
    let buf = t.backend().buffer();
    for y in 0..alto {
        let linea: String = (0..ancho).map(|x| buf[(x, y)].symbol()).collect();
        println!("{}", linea.trim_end());
    }
}
