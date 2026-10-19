//! Fase 2: árbol de carpetas, navegación de la interfaz, exportación y dibujo de pantalla,
//! todo sobre la carpeta de demo.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicU64;

use chrono::TimeZone;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use analizador_de_disco::app::{App, Derecha, Entrada, Exportar, Filtro, Panel};
use analizador_de_disco::arbol::{Arbol, Tipo};
use analizador_de_disco::categoria::Categoria;
use analizador_de_disco::escaner::{Escaneo, Opciones, escanear};
use analizador_de_disco::iconos::Iconos;
use analizador_de_disco::resumen::Resumen;
use analizador_de_disco::{demo, exportar, ui};

fn escanear_demo() -> (tempfile::TempDir, Escaneo) {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("demo");
    demo::crear(&raiz).unwrap();
    let e = escanear(&raiz, &Opciones::default(), &AtomicU64::new(0)).unwrap();
    (dir, e)
}

fn tecla(app: &mut App, code: KeyCode) {
    app.tecla(KeyEvent::from(code));
}

fn escribir(app: &mut App, texto: &str) {
    for c in texto.chars() {
        tecla(app, KeyCode::Char(c));
    }
}

fn nodo(a: &Arbol, e: &Escaneo, relativa: &str) -> usize {
    a.buscar(&e.raiz.join(relativa))
        .unwrap_or_else(|| panic!("no está {relativa}"))
}

fn seleccionar_carpeta(app: &mut App, id: usize) {
    let i = app
        .entradas
        .iter()
        .position(|x| *x == Entrada::Carpeta(id))
        .unwrap();
    tecla(app, KeyCode::Home);
    for _ in 0..i {
        tecla(app, KeyCode::Down);
    }
}

// ───────────────────────────── árbol ─────────────────────────────

#[test]
fn el_arbol_acumula_lo_mismo_que_el_resumen() {
    let (_d, e) = escanear_demo();
    let a = Arbol::de(&e);
    let r = Resumen::de(&e);
    let raiz = a.nodo(Arbol::RAIZ);
    assert_eq!(raiz.usuario_total().cantidad, r.usuario.cantidad);
    assert_eq!(raiz.usuario_total().bytes, r.usuario.bytes);
    assert_eq!(raiz.revisar.bytes, r.revisar_total.bytes);
    for c in Categoria::TODAS {
        assert_eq!(
            raiz.categoria(c).cantidad,
            r.categoria(c).cantidad,
            "{}",
            c.nombre()
        );
    }
}

#[test]
fn el_arbol_suma_cada_subcarpeta() {
    let (_d, e) = escanear_demo();
    let a = Arbol::de(&e);
    let descargas = a.nodo(nodo(&a, &e, "Descargas"));
    assert_eq!(descargas.categoria(Categoria::Instaladores).cantidad, 3);
    assert_eq!(descargas.categoria(Categoria::Comprimidos).cantidad, 2);
    assert_eq!(
        descargas.total().bytes,
        (1024 + 512 + 3072 + 4096 + 700 + 300 + 150 + 500) * 1024
    );
    assert_eq!(a.archivos_bajo(nodo(&a, &e, "Descargas")).len(), 8);
    assert_eq!(a.archivos_bajo(Arbol::RAIZ).len(), e.archivos.len());
}

#[test]
fn el_arbol_marca_proyectos_revisar_y_excluidos() {
    let (_d, e) = escanear_demo();
    let a = Arbol::de(&e);
    assert_eq!(
        a.nodo(nodo(&a, &e, "Proyectos/mi-web")).tipo,
        Tipo::Proyecto
    );
    assert_eq!(a.nodo(nodo(&a, &e, "Proyectos")).tipo, Tipo::Carpeta);
    assert!(a.nodo(nodo(&a, &e, "AppData")).es_solo_revisar());
    assert!(!a.nodo(Arbol::RAIZ).es_solo_revisar());
    let excluidos = a
        .nodo(Arbol::RAIZ)
        .hijos
        .iter()
        .filter(|h| matches!(a.nodo(**h).tipo, Tipo::Excluido(_)))
        .count();
    assert_eq!(excluidos, 6);
}

#[test]
fn archivos_bajo_una_carpeta_son_solo_los_de_adentro() {
    let (_d, e) = escanear_demo();
    let a = Arbol::de(&e);
    let id = nodo(&a, &e, "Imágenes");
    let base = a.ruta(id);
    let bajo = a.archivos_bajo(id);
    assert_eq!(bajo.len(), 5);
    assert!(
        bajo.iter()
            .all(|&i| e.archivos[i as usize].ruta.starts_with(&base))
    );
}

#[test]
fn las_subcarpetas_van_de_mayor_a_menor_y_las_rutas_ida_y_vuelta() {
    let (_d, e) = escanear_demo();
    let a = Arbol::de(&e);
    for (id, n) in a.nodos.iter().enumerate() {
        let tamanos: Vec<u64> = n.hijos.iter().map(|h| a.nodo(*h).total().bytes).collect();
        assert!(tamanos.windows(2).all(|w| w[0] >= w[1]), "{}", n.nombre);
        assert_eq!(a.buscar(&a.ruta(id)), Some(id), "{}", a.ruta(id).display());
    }
}

// ─────────────────────────── navegación ───────────────────────────

#[test]
fn arranca_en_la_raiz_con_la_carpeta_mas_pesada_en_foco() {
    let (_d, e) = escanear_demo();
    let app = App::new(&e, Iconos::Simples);
    let descargas = nodo(&app.arbol, &e, "Descargas");
    assert_eq!(app.actual, Arbol::RAIZ);
    assert_eq!(app.seleccion_izq(), Some(Entrada::Carpeta(descargas)));
    // El panel derecho clasifica la carpeta seleccionada, no toda la raíz.
    assert_eq!(app.foco, descargas);
}

#[test]
fn entrar_y_subir_vuelve_a_seleccionar_la_carpeta_de_la_que_vine() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let descargas = nodo(&app.arbol, &e, "Descargas");

    tecla(&mut app, KeyCode::Enter);
    assert_eq!(app.actual, descargas);
    assert_eq!(app.entradas[0], Entrada::Subir);
    let Some(Entrada::Archivo(i)) = app.seleccion_izq() else {
        panic!("tendría que estar el archivo más pesado");
    };
    assert!(app.archivo(i).ruta.ends_with("ubuntu.iso"));
    assert_eq!(
        app.foco, descargas,
        "con un archivo seleccionado, el foco es la carpeta abierta"
    );

    tecla(&mut app, KeyCode::Backspace);
    assert_eq!(app.actual, Arbol::RAIZ);
    assert_eq!(app.seleccion_izq(), Some(Entrada::Carpeta(descargas)));
}

#[test]
fn en_la_raiz_no_se_puede_subir_ni_entrar_a_lo_excluido() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    tecla(&mut app, KeyCode::Backspace);
    assert_eq!(app.actual, Arbol::RAIZ);

    let windows = nodo(&app.arbol, &e, "Windows");
    seleccionar_carpeta(&mut app, windows);
    tecla(&mut app, KeyCode::Enter);
    assert_eq!(app.actual, Arbol::RAIZ);
}

#[test]
fn los_excluidos_quedan_al_final_de_la_lista() {
    let (_d, e) = escanear_demo();
    let app = App::new(&e, Iconos::Simples);
    let es_excluido = |x: &Entrada| matches!(x, Entrada::Carpeta(id) if matches!(app.arbol.nodo(*id).tipo, Tipo::Excluido(_)));
    let primero = app.entradas.iter().position(es_excluido).unwrap();
    assert!(app.entradas[primero..].iter().all(es_excluido));
}

#[test]
fn desde_una_categoria_se_salta_al_archivo_en_el_disco() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    tecla(&mut app, KeyCode::Tab);
    assert_eq!(app.panel, Panel::Clasificado);

    // El foco es Descargas: bajar hasta Instaladores y abrir.
    for _ in 0..Categoria::Instaladores as usize {
        tecla(&mut app, KeyCode::Down);
    }
    tecla(&mut app, KeyCode::Enter);
    let Derecha::Archivos(c, v) = &app.derecha else {
        panic!("tendría que listar archivos");
    };
    assert_eq!(*c, Some(Filtro::Categoria(Categoria::Instaladores)));
    assert_eq!(v.len(), 3);
    assert!(
        app.archivo(v[0]).ruta.ends_with("ubuntu.iso"),
        "de mayor a menor"
    );

    tecla(&mut app, KeyCode::Enter);
    assert_eq!(app.panel, Panel::Disco);
    assert_eq!(app.actual, nodo(&app.arbol, &e, "Descargas"));
    let Some(Entrada::Archivo(i)) = app.seleccion_izq() else {
        panic!("tendría que quedar seleccionado el archivo");
    };
    assert!(app.archivo(i).ruta.ends_with("ubuntu.iso"));
}

#[test]
fn volver_de_la_lista_de_archivos_a_las_categorias() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    tecla(&mut app, KeyCode::Tab);
    tecla(&mut app, KeyCode::Down);
    tecla(&mut app, KeyCode::Enter);
    assert!(matches!(
        app.derecha,
        Derecha::Archivos(Some(Filtro::Categoria(Categoria::Videos)), _)
    ));
    tecla(&mut app, KeyCode::Left);
    assert!(matches!(app.derecha, Derecha::Categorias));
    assert_eq!(app.tabla_der.selected(), Some(Categoria::Videos as usize));
}

#[test]
fn las_pestanas_se_filtran_por_la_carpeta_en_foco() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);

    tecla(&mut app, KeyCode::Char('6'));
    assert!(
        matches!(&app.derecha, Derecha::Grupos(v) if v.is_empty()),
        "Descargas no tiene proyectos"
    );

    let proyectos = nodo(&app.arbol, &e, "Proyectos");
    seleccionar_carpeta(&mut app, proyectos);
    assert!(matches!(&app.derecha, Derecha::Grupos(v) if v.len() == 2));

    let appdata = nodo(&app.arbol, &e, "AppData");
    seleccionar_carpeta(&mut app, appdata);
    tecla(&mut app, KeyCode::Char('7'));
    assert!(matches!(&app.derecha, Derecha::Grupos(v) if v.len() == 4));

    tecla(&mut app, KeyCode::Char('8'));
    assert!(matches!(&app.derecha, Derecha::Excluidos(v) if v.is_empty()));
}

#[test]
fn desde_revisar_se_abre_la_carpeta_del_programa() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let appdata = nodo(&app.arbol, &e, "AppData");
    seleccionar_carpeta(&mut app, appdata);
    tecla(&mut app, KeyCode::Char('7'));
    tecla(&mut app, KeyCode::Tab);
    tecla(&mut app, KeyCode::Enter);
    // El más pesado es WhatsApp (600 KB).
    assert_eq!(app.actual, nodo(&app.arbol, &e, "AppData/Roaming/WhatsApp"));
}

#[test]
fn las_fechas_del_detalle_son_las_de_la_categoria_en_foco() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let documentos = nodo(&app.arbol, &e, "Documentos");
    seleccionar_carpeta(&mut app, documentos);
    tecla(&mut app, KeyCode::Tab);
    for _ in 0..Categoria::Documentos as usize {
        tecla(&mut app, KeyCode::Down);
    }
    let t = app.detalle.clone().expect("hay detalle");
    assert_eq!(t.cantidad, 6);
    let anio = chrono::DateTime::<chrono::Utc>::from(t.mas_viejo.unwrap())
        .format("%Y")
        .to_string();
    assert_eq!(anio, "2009");
}

#[test]
fn q_sale_pero_no_mientras_se_escribe_el_destino() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    tecla(&mut app, KeyCode::Char('e'));
    escribir(&mut app, "q");
    assert!(!app.salir);
    assert!(matches!(&app.exportar, Some(Exportar::Destino(d)) if d.ends_with('q')));
    tecla(&mut app, KeyCode::Esc);
    assert!(app.exportar.is_none());
    tecla(&mut app, KeyCode::Char('q'));
    assert!(app.salir);
}

#[test]
fn exportar_desde_la_interfaz_con_vista_previa() {
    let (_d, e) = escanear_demo();
    let destino = tempfile::tempdir().unwrap();
    let mut app = App::new(&e, Iconos::Simples);

    tecla(&mut app, KeyCode::Char('e'));
    let Some(Exportar::Destino(d)) = &app.exportar else {
        panic!("tendría que pedir el destino");
    };
    for _ in 0..d.chars().count() {
        tecla(&mut app, KeyCode::Backspace);
    }
    escribir(&mut app, &destino.path().display().to_string());
    tecla(&mut app, KeyCode::Enter);

    let Some(Exportar::Confirmar { plan, sobrescribe }) = app.exportar.clone() else {
        panic!("tendría que mostrar la vista previa");
    };
    assert!(!sobrescribe);
    assert!(!plan.markdown.exists(), "la vista previa no escribe nada");

    tecla(&mut app, KeyCode::Enter);
    assert!(
        matches!(app.exportar, Some(Exportar::Listo(_))),
        "{:?}",
        app.exportar
    );
    assert!(plan.markdown.exists());
    assert!(plan.csv.exists());
    tecla(&mut app, KeyCode::Char('x'));
    assert!(app.exportar.is_none());
}

#[test]
fn cancelar_la_vista_previa_no_escribe_nada() {
    let (_d, e) = escanear_demo();
    let destino = tempfile::tempdir().unwrap();
    let mut app = App::new(&e, Iconos::Simples);
    let plan = exportar::planificar(destino.path(), &e, chrono::Local::now());
    app.exportar = Some(Exportar::Confirmar {
        plan,
        sobrescribe: false,
    });
    tecla(&mut app, KeyCode::Char('n'));
    assert!(app.exportar.is_none());
    assert_eq!(fs::read_dir(destino.path()).unwrap().count(), 0);
}

// ─────────────────────────── exportación ───────────────────────────

#[test]
fn csv_con_bom_encabezado_y_una_fila_por_archivo() {
    let (_d, e) = escanear_demo();
    let mut out = Vec::new();
    exportar::csv(&e, None, &mut out).unwrap();
    let texto = String::from_utf8(out).unwrap();
    assert!(texto.starts_with('\u{feff}'), "BOM para Excel");
    let lineas: Vec<&str> = texto
        .trim_start_matches('\u{feff}')
        .split("\r\n")
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        lineas[0],
        "ruta;categoria;zona;proyecto;carpeta_revisar;bytes;tamano;modificado;borrable;grupo_duplicado"
    );
    assert_eq!(lineas.len(), e.archivos.len() + 1);
    assert!(
        lineas.iter().skip(1).all(|l| l.split(';').count() == 10),
        "10 columnas en cada fila"
    );

    let foto = lineas.iter().find(|l| l.contains("IMG_0002.JPG")).unwrap();
    assert!(
        foto.contains(";Fotos;usuario;;;256000;250,0 KB;2019-01-01"),
        "{foto}"
    );
    let whatsapp = lineas.iter().find(|l| l.contains("msgstore.db")).unwrap();
    assert!(whatsapp.contains(";revisar;"), "{whatsapp}");
    assert!(
        whatsapp.contains("WhatsApp;614400;"),
        "la carpeta de revisar va en su columna: {whatsapp}"
    );
    let logo = lineas.iter().find(|l| l.contains("logo.png")).unwrap();
    assert!(
        logo.contains(";Proyectos/código;usuario;") && logo.contains("mi-web;"),
        "{logo}"
    );
}

#[test]
fn csv_con_nombres_complicados() {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("raros");
    fs::create_dir_all(&raiz).unwrap();
    fs::write(raiz.join("uno;dos.txt"), "x").unwrap();
    fs::write(raiz.join("canción ñandú.mp3"), "x").unwrap();
    // Windows no permite comillas en los nombres; en Linux sí.
    #[cfg(unix)]
    fs::write(raiz.join("el \"mejor\".doc"), "x").unwrap();
    let e = escanear(&raiz, &Opciones::default(), &AtomicU64::new(0)).unwrap();
    let mut out = Vec::new();
    exportar::csv(&e, None, &mut out).unwrap();
    let texto = String::from_utf8(out).unwrap();
    assert!(texto.contains("uno;dos.txt\";Documentos;"), "{texto}");
    assert!(texto.contains("canción ñandú.mp3;Música;"), "{texto}");
    #[cfg(unix)]
    assert!(
        texto.contains("el \"\"mejor\"\".doc\";Documentos;"),
        "{texto}"
    );
}

#[test]
fn markdown_con_todas_las_secciones() {
    let (_d, e) = escanear_demo();
    let mut out = Vec::new();
    exportar::markdown(&e, None, &mut out).unwrap();
    let md = String::from_utf8(out).unwrap();
    for esperado in [
        "# Informe de disco:",
        "Escaneo de 43 archivos en",
        "## Por categoría",
        "| Fotos | 7 | 1,6 MB |",
        "| Instaladores | 4 |",
        "## Más pesados por categoría",
        "### Videos",
        "## Proyectos de código (2)",
        "## Revisar: datos de programas (4 archivos",
        "## Excluido por ser del sistema (6)",
        "| `Windows` | sistema Windows |",
    ] {
        assert!(md.contains(esperado), "falta {esperado:?} en:\n{md}");
    }
}

#[test]
fn el_plan_arma_nombres_con_carpeta_y_fecha() {
    let (_d, e) = escanear_demo();
    let ahora = chrono::Local
        .with_ymd_and_hms(2026, 10, 19, 14, 30, 0)
        .unwrap();
    let plan = exportar::planificar(Path::new("salida"), &e, ahora);
    assert_eq!(
        plan.markdown,
        Path::new("salida").join("informe-demo-2026-10-19_1430.md")
    );
    assert_eq!(
        plan.csv,
        Path::new("salida").join("informe-demo-2026-10-19_1430.csv")
    );
    assert_eq!(plan.filas_csv, e.archivos.len());
}

#[test]
fn exportar_no_pisa_archivos_sin_permiso() {
    let (_d, e) = escanear_demo();
    let destino = tempfile::tempdir().unwrap();
    let plan = exportar::planificar(destino.path(), &e, chrono::Local::now());
    fs::write(&plan.csv, "viejo").unwrap();

    assert_eq!(plan.existentes(), vec![plan.csv.as_path()]);
    assert!(exportar::escribir(&plan, &e, None, false).is_err());
    assert_eq!(fs::read_to_string(&plan.csv).unwrap(), "viejo");
    assert!(
        !plan.markdown.exists(),
        "si falla, no escribe ninguno de los dos"
    );

    exportar::escribir(&plan, &e, None, true).unwrap();
    assert_ne!(fs::read_to_string(&plan.csv).unwrap(), "viejo");
}

#[test]
fn exportar_crea_la_carpeta_de_destino() {
    let (_d, e) = escanear_demo();
    let destino = tempfile::tempdir().unwrap();
    let plan = exportar::planificar(
        &destino.path().join("informes/2026"),
        &e,
        chrono::Local::now(),
    );
    exportar::escribir(&plan, &e, None, false).unwrap();
    assert!(plan.markdown.exists());
}

// ───────────────────────────── pantalla ─────────────────────────────

fn pantalla(app: &mut App, ancho: u16, alto: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(ancho, alto)).unwrap();
    t.draw(|f| ui::dibujar(f, app)).unwrap();
    let buf = t.backend().buffer();
    let mut s = String::new();
    for y in 0..alto {
        for x in 0..ancho {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}

#[test]
fn la_pantalla_muestra_los_dos_paneles() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let p = pantalla(&mut app, 140, 34);
    for esperado in [
        " analizador ",
        "Disco",
        "Clasificado · Descargas",
        "► Descargas",
        "♦ AppData  (revisar)",
        "× Windows  (sistema)",
        "■ Grabaciones de voz",
        "■ Proyectos/código",
        "1 Categorías",
        "■ Instaladores",
        "Enter entrar",
        "e informe",
    ] {
        assert!(p.contains(esperado), "falta {esperado:?} en:\n{p}");
    }
}

#[test]
fn la_pantalla_muestra_la_vista_previa_de_exportar() {
    let (_d, e) = escanear_demo();
    let mut app = App::new(&e, Iconos::Simples);
    let plan = exportar::planificar(Path::new("salida"), &e, chrono::Local::now());
    app.exportar = Some(Exportar::Confirmar {
        plan,
        sobrescribe: true,
    });
    let p = pantalla(&mut app, 140, 34);
    assert!(p.contains("Vista previa. Se van a crear:"), "{p}");
    assert!(p.contains("se van a sobrescribir"), "{p}");
}

#[test]
fn la_pantalla_aguanta_terminales_chicas_y_emoji() {
    let (_d, e) = escanear_demo();
    for iconos in [Iconos::Simples, Iconos::Emoji, Iconos::Nerd] {
        let mut app = App::new(&e, iconos);
        for (ancho, alto) in [(20, 5), (60, 12), (200, 60)] {
            pantalla(&mut app, ancho, alto);
        }
        tecla(&mut app, KeyCode::Enter);
        pantalla(&mut app, 80, 24);
    }
}

#[test]
fn la_pantalla_de_progreso() {
    let mut t = Terminal::new(TestBackend::new(80, 12)).unwrap();
    t.draw(|f| {
        ui::dibujar_progreso(
            f,
            Path::new("C:/"),
            123_456,
            std::time::Duration::from_secs(2),
        )
    })
    .unwrap();
    let buf = t.backend().buffer();
    let texto: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(texto.contains("Escaneando"));
    assert!(texto.contains("123.456"));
    assert!(texto.contains("61.728"), "archivos por segundo");
}
