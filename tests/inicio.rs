//! Pantalla de inicio (doble click): qué se ve, con las unidades reales de esta máquina.

use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;

use analizador_de_disco::inicio::{Eleccion, Inicio, Opcion, carpeta_personal};
use analizador_de_disco::{ui, unidades};

fn pantalla(inicio: &mut Inicio, ancho: u16, alto: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(ancho, alto)).unwrap();
    t.draw(|f| ui::dibujar_inicio(f, inicio)).unwrap();
    let buf = t.backend().buffer();
    (0..alto)
        .map(|y| (0..ancho).map(|x| buf[(x, y)].symbol()).collect::<String>() + "\n")
        .collect()
}

#[test]
fn muestra_la_carpeta_personal_los_discos_y_la_demo() {
    let mut ini = Inicio::new(unidades::detectar(), carpeta_personal());
    let p = pantalla(&mut ini, 110, 30);
    for esperado in [
        "¿Qué querés revisar?",
        "Mi carpeta personal",
        "Disco",
        "libres",
        "Probar con la demo",
        "Otra carpeta…",
        "q salir",
    ] {
        assert!(p.contains(esperado), "falta {esperado:?} en:\n{p}");
    }
    assert!(p.contains(env!("CARGO_PKG_VERSION")), "{p}");
}

#[test]
fn la_carpeta_personal_es_la_primera_y_se_abre_con_enter() {
    let personal = carpeta_personal().expect("en los tests hay carpeta personal");
    let mut ini = Inicio::new(unidades::detectar(), Some(personal.clone()));
    assert_eq!(ini.opciones[0], Opcion::Personal(personal.clone()));
    assert_eq!(ini.tecla(KeyCode::Enter), Some(Eleccion::Abrir(personal)));
}

#[test]
fn escribir_otra_carpeta_se_ve_y_un_error_tambien() {
    let mut ini = Inicio::new(Vec::new(), None);
    ini.tecla(KeyCode::End);
    ini.tecla(KeyCode::Enter);
    for c in "D:\\Fotos".chars() {
        ini.tecla(KeyCode::Char(c));
    }
    let p = pantalla(&mut ini, 100, 20);
    assert!(p.contains("> D:\\Fotos"), "{p}");

    ini.tecla(KeyCode::Esc);
    ini.mensaje = Some(format!(
        "{} no existe o no es una carpeta",
        PathBuf::from("X:\\nada").display()
    ));
    let p = pantalla(&mut ini, 100, 20);
    assert!(p.contains("no existe o no es una carpeta"), "{p}");
}

#[test]
fn aguanta_terminales_chicas() {
    let mut ini = Inicio::new(unidades::detectar(), carpeta_personal());
    for (ancho, alto) in [(20, 5), (40, 10), (80, 24)] {
        pantalla(&mut ini, ancho, alto);
    }
}
