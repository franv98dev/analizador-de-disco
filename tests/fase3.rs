//! Fase 3: duplicados, "probablemente borrable", más pesados y antigüedad, sobre la demo.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use analizador_de_disco::app::{App, Derecha, Entrada, Filtro, Panel};
use analizador_de_disco::borrable::{self, Motivo};
use analizador_de_disco::duplicados::{self, Duplicados, Progreso, TAMANO_MINIMO};
use analizador_de_disco::escaner::{Escaneo, Opciones, escanear};
use analizador_de_disco::iconos::Iconos;
use analizador_de_disco::{demo, exportar, ui};

fn escanear_demo() -> (tempfile::TempDir, Escaneo) {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("demo");
    demo::crear(&raiz).unwrap();
    let e = escanear(&raiz, &Opciones::default(), &AtomicU64::new(0)).unwrap();
    (dir, e)
}

fn buscar(e: &Escaneo, hilos: Option<usize>) -> Duplicados {
    duplicados::buscar(duplicados::candidatos(e), &Progreso::default(), hilos).unwrap()
}

fn rel(e: &Escaneo, i: u32) -> String {
    e.archivos[i as usize]
        .ruta
        .strip_prefix(&e.raiz)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn grupos_como_texto(e: &Escaneo, d: &Duplicados) -> BTreeSet<Vec<String>> {
    d.grupos
        .iter()
        .map(|g| g.archivos.iter().map(|&a| rel(e, a)).collect())
        .collect()
}

fn tecla(app: &mut App, code: KeyCode) {
    app.tecla(KeyEvent::from(code));
}

fn esperar_duplicados(app: &mut App) {
    let inicio = Instant::now();
    while app.busqueda.is_some() {
        assert!(
            inicio.elapsed() < Duration::from_secs(30),
            "la búsqueda no termina"
        );
        std::thread::sleep(Duration::from_millis(5));
        app.tick();
    }
}

// ─────────────────────────── duplicados ───────────────────────────

#[test]
fn encuentra_las_copias_y_sugiere_conservar_la_mas_vieja() {
    let (_d, e) = escanear_demo();
    let d = buscar(&e, None);
    let esperado: BTreeSet<Vec<String>> = [
        vec![
            "Documentos/CV.pdf",
            "Documentos/CV copia.pdf",
            "Descargas/CV.pdf",
        ],
        vec![
            "Imágenes/Vacaciones 2019/IMG_0001.jpg",
            "Descargas/IMG_0001 (1).jpg",
        ],
        vec!["Videos/cumple de la abuela.mp4", "Escritorio/cumple.mp4"],
    ]
    .into_iter()
    .map(|g| g.into_iter().map(String::from).collect())
    .collect();
    assert_eq!(grupos_como_texto(&e, &d), esperado);
    assert_eq!(d.copias(), 4);
    assert_eq!(d.liberable(), (2048 + 300 + 150 * 2) * 1024);
    assert!(d.errores.is_empty());
}

#[test]
fn los_grupos_van_de_mayor_a_menor_espacio_liberable() {
    let (_d, e) = escanear_demo();
    let d = buscar(&e, None);
    assert_eq!(
        rel(&e, d.grupos[0].archivos[0]),
        "Videos/cumple de la abuela.mp4"
    );
    assert!(
        d.grupos
            .windows(2)
            .all(|w| w[0].liberable() >= w[1].liberable())
    );
}

#[test]
fn mismo_principio_y_final_pero_distinto_en_el_medio_no_es_duplicado() {
    let (_d, e) = escanear_demo();
    let d = buscar(&e, None);
    let todos: Vec<String> = d
        .grupos
        .iter()
        .flat_map(|g| g.archivos.iter().map(|&a| rel(&e, a)))
        .collect();
    assert!(!todos.iter().any(|r| r.contains("editada")), "{todos:?}");
    // Y el juego portable pesa lo mismo que IMG_0001 pero es otra cosa.
    assert!(!todos.iter().any(|r| r.contains("juego.exe")), "{todos:?}");
}

#[test]
fn no_mira_revisar_ni_proyectos_ni_archivos_chicos() {
    let (_d, e) = escanear_demo();
    for c in duplicados::candidatos(&e) {
        let a = &e.archivos[c.indice as usize];
        assert!(
            a.revisar.is_none() && a.proyecto.is_none(),
            "{}",
            c.ruta.display()
        );
        assert!(c.tamano >= TAMANO_MINIMO);
    }
}

#[test]
fn con_un_hilo_da_lo_mismo() {
    let (_d, e) = escanear_demo();
    assert_eq!(buscar(&e, Some(1)).grupos, buscar(&e, None).grupos);
}

#[test]
fn buscar_duplicados_no_modifica_nada() {
    fn foto(raiz: &Path) -> BTreeSet<(PathBuf, u64, SystemTime)> {
        let mut v = BTreeSet::new();
        let mut pendientes = vec![raiz.to_path_buf()];
        while let Some(d) = pendientes.pop() {
            for e in fs::read_dir(&d).unwrap() {
                let ruta = e.unwrap().path();
                let m = fs::symlink_metadata(&ruta).unwrap();
                if m.is_dir() {
                    pendientes.push(ruta);
                } else {
                    v.insert((ruta, m.len(), m.modified().unwrap()));
                }
            }
        }
        v
    }
    let (_d, e) = escanear_demo();
    let antes = foto(&e.raiz);
    buscar(&e, None);
    assert_eq!(antes, foto(&e.raiz));
}

#[test]
fn si_un_archivo_cambio_desde_el_escaneo_no_se_compara() {
    let (_d, e) = escanear_demo();
    let copia = e.raiz.join("Escritorio/cumple.mp4");
    OpenOptions::new()
        .append(true)
        .open(&copia)
        .unwrap()
        .write_all(b"cambio")
        .unwrap();
    let d = buscar(&e, None);
    assert_eq!(
        d.grupos.len(),
        2,
        "el grupo del video ya no se puede confirmar"
    );
    assert_eq!(d.errores.len(), 1);
    assert!(
        d.errores[0].mensaje.contains("cambió"),
        "{}",
        d.errores[0].mensaje
    );
}

#[test]
fn se_puede_cancelar() {
    let (_d, e) = escanear_demo();
    let p = Progreso::default();
    p.cancelar.store(true, Ordering::Relaxed);
    let d = duplicados::buscar(duplicados::candidatos(&e), &p, None).unwrap();
    assert!(d.cancelado);
    assert!(d.grupos.is_empty());
    assert!(d.errores.is_empty(), "cancelar no es un error de lectura");
}

// ─────────────────────── probablemente borrable ───────────────────────

#[test]
fn borrable_por_motivo_en_la_demo() {
    let (_d, e) = escanear_demo();
    let t = borrable::totales(&borrable::analizar(&e, None));
    assert_eq!(
        t[&Motivo::Temporal].cantidad,
        5,
        "~$, .crdownload, .tmp, Thumbs.db y AppData/Local/Temp"
    );
    assert_eq!(t[&Motivo::Cache].cantidad, 1, "node_modules");
    assert_eq!(
        t[&Motivo::Instalador].cantidad,
        3,
        "setup.exe, ubuntu.iso, driver.msi (no el portable)"
    );
    assert!(
        !t.contains_key(&Motivo::Duplicado),
        "sin buscar duplicados no hay copias"
    );
}

#[test]
fn con_duplicados_se_suman_las_copias_pero_no_el_original() {
    let (_d, e) = escanear_demo();
    let d = buscar(&e, None);
    let items = borrable::analizar(&e, Some(&d));
    let copias: BTreeSet<String> = items
        .iter()
        .filter(|i| i.motivo == Motivo::Duplicado)
        .map(|i| rel(&e, i.archivo.unwrap()))
        .collect();
    let esperado: BTreeSet<String> = [
        "Documentos/CV copia.pdf",
        "Descargas/CV.pdf",
        "Descargas/IMG_0001 (1).jpg",
        "Escritorio/cumple.mp4",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(copias, esperado);
}

#[test]
fn temporales_y_caches_se_agrupan_por_carpeta() {
    let (_d, e) = escanear_demo();
    let items = borrable::analizar(&e, None);
    let carpetas: BTreeSet<String> = items
        .iter()
        .filter(|i| i.archivo.is_none())
        .map(|i| {
            i.ruta
                .strip_prefix(&e.raiz)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let esperado: BTreeSet<String> = ["AppData/Local/Temp", "Proyectos/mi-web/node_modules"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(carpetas, esperado);
}

// ─────────────────────────── interfaz ───────────────────────────

#[test]
fn pestana_pesados_lista_todo_de_mayor_a_menor() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    app.ir_a(&e.raiz, None);
    tecla(&mut app, KeyCode::Backspace); // ya en la raíz: no hace nada
    // Foco en la raíz: seleccionar el archivo LEEME para que el foco sea la carpeta abierta.
    let leeme = app
        .entradas
        .iter()
        .position(|x| matches!(x, Entrada::Archivo(_)))
        .unwrap();
    for _ in 0..leeme {
        tecla(&mut app, KeyCode::Down);
    }
    tecla(&mut app, KeyCode::Char('2'));
    let Derecha::Archivos(None, v) = &app.derecha else {
        panic!("tendría que ser la lista de pesados");
    };
    assert_eq!(rel(&e, v[0]), "Descargas/ubuntu.iso");
    assert_eq!(v.len(), e.archivos.len());
    assert!(
        v.windows(2)
            .all(|w| app.archivo(w[0]).tamano >= app.archivo(w[1]).tamano)
    );
}

#[test]
fn antiguedad_por_anio_y_volver_al_mismo_anio() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let documentos = app.arbol.buscar(&e.raiz.join("Documentos")).unwrap();
    let i = app
        .entradas
        .iter()
        .position(|x| *x == Entrada::Carpeta(documentos))
        .unwrap();
    tecla(&mut app, KeyCode::Home);
    for _ in 0..i {
        tecla(&mut app, KeyCode::Down);
    }
    tecla(&mut app, KeyCode::Char('3'));
    let Derecha::Anios(v) = &app.derecha else {
        panic!("tendría que mostrar años");
    };
    assert_eq!(v[0].0, Some(2009), "el más viejo primero");
    assert_eq!(v[0].1.cantidad, 1);

    tecla(&mut app, KeyCode::Tab);
    tecla(&mut app, KeyCode::Home);
    tecla(&mut app, KeyCode::Enter);
    let Derecha::Archivos(Some(Filtro::Anio(Some(2009))), v) = &app.derecha else {
        panic!("tendría que listar los de 2009: {:?}", app.derecha);
    };
    assert_eq!(v.len(), 1);
    assert_eq!(rel(&e, v[0]), "Documentos/CV.pdf");

    tecla(&mut app, KeyCode::Left);
    assert!(matches!(app.derecha, Derecha::Anios(_)));
    assert_eq!(app.tabla_der.selected(), Some(0));
}

#[test]
fn buscar_duplicados_desde_la_interfaz_y_saltar_a_una_copia() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    // Foco en Descargas (la carpeta más pesada): tiene una copia del CV y una de la foto.
    tecla(&mut app, KeyCode::Char('d'));
    assert_eq!(app.pestana, analizador_de_disco::app::Pestana::Duplicados);
    esperar_duplicados(&mut app);
    assert!(app.duplicados.is_some());

    let Derecha::Duplicados(v) = &app.derecha else {
        panic!("tendría que mostrar los duplicados");
    };
    assert_eq!(v.len(), 2, "solo los grupos con algo en Descargas");

    tecla(&mut app, KeyCode::Tab);
    tecla(&mut app, KeyCode::Enter);
    assert!(matches!(app.derecha, Derecha::Copias(_)));
    tecla(&mut app, KeyCode::Down);
    tecla(&mut app, KeyCode::Enter);
    assert_eq!(app.panel, Panel::Disco);
    let Some(Entrada::Archivo(i)) = app.seleccion_izq() else {
        panic!("tendría que quedar seleccionada la copia");
    };
    assert!(rel(&e, i).starts_with("Descargas/") || rel(&e, i).starts_with("Documentos/"));
}

#[test]
fn despues_de_buscar_duplicados_el_borrable_suma_las_copias() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let antes = borrable::totales(&app.borrable);
    tecla(&mut app, KeyCode::Char('d'));
    esperar_duplicados(&mut app);
    let despues = borrable::totales(&app.borrable);
    assert!(!antes.contains_key(&Motivo::Duplicado));
    assert_eq!(despues[&Motivo::Duplicado].cantidad, 4);
}

#[test]
fn d_durante_la_busqueda_la_cancela() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    tecla(&mut app, KeyCode::Char('d'));
    tecla(&mut app, KeyCode::Char('d'));
    esperar_duplicados(&mut app);
    // Puede haber terminado antes de cancelar (la demo es chica): las dos salidas son válidas.
    assert!(app.duplicados.is_some() || app.aviso.as_deref() == Some("Búsqueda cancelada."));
}

fn pantalla(app: &mut App) -> String {
    let (ancho, alto) = (140, 34);
    let mut t = Terminal::new(TestBackend::new(ancho, alto)).unwrap();
    t.draw(|f| ui::dibujar(f, app)).unwrap();
    let buf = t.backend().buffer();
    (0..alto)
        .map(|y| (0..ancho).map(|x| buf[(x, y)].symbol()).collect::<String>() + "\n")
        .collect()
}

#[test]
fn la_pantalla_de_borrable_y_de_duplicados() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    tecla(&mut app, KeyCode::Char('4'));
    let p = pantalla(&mut app);
    for esperado in [
        "4 Borrable",
        "Instalador",
        "ubuntu.iso",
        "solo sugerencias",
        "d: sumar copias",
    ] {
        assert!(p.contains(esperado), "falta {esperado:?} en:\n{p}");
    }

    tecla(&mut app, KeyCode::Char('5'));
    let p = pantalla(&mut app);
    assert!(p.contains("Tocá d para buscar"), "{p}");

    tecla(&mut app, KeyCode::Char('d'));
    esperar_duplicados(&mut app);
    tecla(&mut app, KeyCode::Tab);
    tecla(&mut app, KeyCode::Enter);
    let p = pantalla(&mut app);
    assert!(p.contains("conservar"), "{p}");
    assert!(p.contains("copia"), "{p}");
}

// ─────────────────────────── informe ───────────────────────────

#[test]
fn el_informe_incluye_borrable_antiguedad_y_duplicados() {
    let (_d, e) = escanear_demo();
    let d = buscar(&e, None);
    let mut out = Vec::new();
    exportar::markdown(&e, Some(&d), &mut out).unwrap();
    let md = String::from_utf8(out).unwrap();
    for esperado in [
        "## Probablemente borrable",
        "| Instalador |",
        "## Duplicados",
        "se liberan **2,6 MB**",
        "`Videos\\cumple de la abuela.mp4` (conservar)",
        "## Antigüedad (última modificación)",
        "| 2009 | 1 |",
    ] {
        let esperado = esperado.replace('\\', std::path::MAIN_SEPARATOR_STR);
        assert!(md.contains(&esperado), "falta {esperado:?} en:\n{md}");
    }
}

#[test]
fn el_informe_sin_duplicados_explica_como_pedirlos() {
    let (_d, e) = escanear_demo();
    let mut out = Vec::new();
    exportar::markdown(&e, None, &mut out).unwrap();
    let md = String::from_utf8(out).unwrap();
    assert!(md.contains("No se buscaron."), "{md}");
    assert!(md.contains("--duplicados"), "{md}");
}

#[test]
fn el_csv_marca_borrable_y_grupo_de_duplicados() {
    let (_d, e) = escanear_demo();
    let d = buscar(&e, None);
    let mut out = Vec::new();
    exportar::csv(&e, Some(&d), &mut out).unwrap();
    let csv = String::from_utf8(out).unwrap();
    let linea = |nombre: &str| -> String {
        csv.lines()
            .find(|l| l.contains(nombre))
            .unwrap_or_else(|| panic!("no está {nombre}"))
            .to_string()
    };
    assert!(
        linea("cumple de la abuela.mp4").ends_with(";;1 (conservar)"),
        "{}",
        linea("cumple de la abuela")
    );
    assert!(
        linea("Escritorio").ends_with(";Copia;1"),
        "{}",
        linea("Escritorio")
    );
    assert!(
        linea("ubuntu.iso").ends_with(";Instalador;"),
        "{}",
        linea("ubuntu.iso")
    );
    assert!(
        linea("juego.exe").ends_with(";;"),
        "el portable no es borrable: {}",
        linea("juego.exe")
    );
}
