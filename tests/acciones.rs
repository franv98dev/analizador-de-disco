//! Fase 4: ordenar (mover/copiar), `_para_borrar` y deshacer, sobre la demo.
//! Es la parte que escribe y mueve archivos: los tests verifican sobre todo lo que NO
//! tiene que pasar (pisar, perder, dejar a medias).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use chrono::Local;

use analizador_de_disco::acciones::{self, Elemento, Modo, OpcionesOrdenar, ProgresoAccion, Tipo};
use analizador_de_disco::borrable;
use analizador_de_disco::demo;
use analizador_de_disco::duplicados::{self, Progreso};
use analizador_de_disco::escaner::{Escaneo, Opciones, escanear};

fn escanear_demo() -> (tempfile::TempDir, Escaneo) {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("demo");
    demo::crear(&raiz).unwrap();
    let e = reescanear(&raiz);
    (dir, e)
}

fn reescanear(raiz: &Path) -> Escaneo {
    escanear(raiz, &Opciones::default(), &AtomicU64::new(0)).unwrap()
}

/// Todo lo del usuario (sin "revisar"), como lo elegiría `analizador ordenar`.
fn lo_del_usuario(e: &Escaneo) -> Vec<Elemento> {
    (0..e.archivos.len() as u32)
        .filter(|&i| e.archivos[i as usize].revisar.is_none())
        .map(|i| Elemento::de(e, i))
        .collect()
}

/// Ruta → (tamaño, fecha, hash) de todos los archivos debajo de `raiz`.
fn foto(raiz: &Path) -> BTreeMap<PathBuf, (u64, SystemTime, blake3::Hash)> {
    let mut m = BTreeMap::new();
    let mut pendientes = vec![raiz.to_path_buf()];
    while let Some(d) = pendientes.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd {
            let ruta = e.unwrap().path();
            let meta = fs::symlink_metadata(&ruta).unwrap();
            if meta.is_dir() {
                pendientes.push(ruta);
            } else {
                let hash = blake3::hash(&fs::read(&ruta).unwrap());
                m.insert(ruta, (meta.len(), meta.modified().unwrap(), hash));
            }
        }
    }
    m
}

fn relativas(base: &Path, rutas: impl Iterator<Item = PathBuf>) -> BTreeSet<String> {
    rutas
        .map(|r| {
            r.strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn parciales(raiz: &Path) -> Vec<PathBuf> {
    foto(raiz)
        .into_keys()
        .filter(|p| p.to_string_lossy().ends_with(".parcial"))
        .collect()
}

fn ordenar(e: &Escaneo, destino: &Path, modo: Modo, dups: bool) -> acciones::Plan {
    let d = dups.then(|| {
        duplicados::buscar(duplicados::candidatos(e), &Progreso::default(), None).unwrap()
    });
    acciones::planificar_ordenar(
        &lo_del_usuario(e),
        &e.raiz,
        destino,
        OpcionesOrdenar {
            modo: Some(modo),
            incluir_instaladores: false,
        },
        d.as_ref(),
        Local::now(),
    )
}

/// Una carpeta de sistema de verdad en cada sistema operativo.
fn destino_de_sistema() -> &'static Path {
    if cfg!(windows) {
        Path::new(r"C:\Windows\Ordenado")
    } else {
        Path::new("/usr/local/Ordenado")
    }
}

// ───────────────────────────── dry-run ─────────────────────────────

#[test]
fn planificar_no_escribe_nada() {
    let (dir, e) = escanear_demo();
    let antes = foto(dir.path());
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    assert!(plan.se_puede_ejecutar(), "{:?}", plan.problemas);
    assert!(!destino.exists(), "el dry-run no crea ni la carpeta");
    acciones::planificar_para_borrar(&lo_del_usuario(&e), &e.raiz);
    assert_eq!(antes, foto(dir.path()));
}

#[test]
fn el_plan_ordena_como_se_pidio() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    let destinos = relativas(&destino, plan.operaciones.iter().map(|o| o.destino.clone()));
    for esperado in [
        "Fotos/2015/IMG_20150704_120000.jpg", // por EXIF, aunque se modificó en 2023
        "Fotos/2019/IMG_0001.jpg",
        "Fotos/2023/20230405_101010.heic",
        "Videos/2015/cumple de la abuela.mp4",
        "Grabaciones de voz/2020/Grabación 001.m4a",
        "Documentos/CV.pdf",
        "Documentos/Windows/trucos de windows.docx",
        "Música/Soda Stereo/De música ligera.mp3",
        "Proyectos/mi-web/src/index.js",
        "Proyectos/juego/.git/HEAD",
        "Comprimidos/fotos-viejas.zip",
        "Otros/Varios/archivo.xyz",
    ] {
        assert!(
            destinos.contains(esperado),
            "falta {esperado} en {destinos:#?}"
        );
    }
    assert_eq!(
        plan.omitidos_instaladores, 4,
        "los instaladores no se ordenan salvo que se pida"
    );
    assert_eq!(
        plan.omitidos_duplicados, 4,
        "de cada grupo de duplicados va solo uno"
    );
    assert_eq!(
        plan.omitidos_temporales, 4,
        "~$ de Office, .crdownload, .tmp y Thumbs.db no son algo que se conserve"
    );
    assert!(
        !destinos
            .iter()
            .any(|d| d.contains("Thumbs.db") || d.contains("~$"))
    );
    assert!(!destinos.iter().any(|d| d.contains("CV copia")));
    assert!(plan.avisos.is_empty(), "{:?}", plan.avisos);
}

#[test]
fn sin_buscar_duplicados_avisa_y_renombra_los_choques() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, false);
    assert!(plan.avisos.iter().any(|a| a.contains("duplicados")));
    let destinos = relativas(&destino, plan.operaciones.iter().map(|o| o.destino.clone()));
    // Documentos/CV.pdf y Descargas/CV.pdf van al mismo lugar: uno queda como "CV (2).pdf".
    assert!(destinos.contains("Documentos/CV.pdf"));
    assert!(destinos.contains("Documentos/CV (2).pdf"), "{destinos:#?}");
    assert!(plan.renombrados >= 1);
}

#[test]
fn no_acepta_destinos_de_sistema() {
    let (_dir, e) = escanear_demo();
    let plan = ordenar(&e, destino_de_sistema(), Modo::Mover, false);
    assert!(!plan.se_puede_ejecutar());
    assert!(acciones::ejecutar(&plan, &ProgresoAccion::default()).is_err());
}

#[test]
fn avisa_si_no_hay_espacio() {
    let (dir, e) = escanear_demo();
    let mut grande = Elemento::de(&e, 0);
    grande.tamano = u64::MAX / 4;
    grande.categoria = analizador_de_disco::categoria::Categoria::Documentos;
    let plan = acciones::planificar_ordenar(
        &[grande],
        &e.raiz,
        &dir.path().join("Ordenado"),
        OpcionesOrdenar {
            modo: Some(Modo::Copiar),
            incluir_instaladores: false,
        },
        None,
        Local::now(),
    );
    assert!(
        plan.problemas.iter().any(|p| p.contains("no hay espacio")),
        "{:?}",
        plan.problemas
    );
}

// ─────────────────────────── ejecutar ───────────────────────────

#[test]
fn mover_lleva_todo_al_destino_con_el_mismo_contenido_y_fecha() {
    let (dir, e) = escanear_demo();
    let antes = foto(&e.raiz);
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    let r = acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    assert_eq!(r.hechos, plan.operaciones.len());
    assert!(r.fallidos.is_empty(), "{:?}", r.fallidos);

    for op in &plan.operaciones {
        assert!(
            !op.origen.exists(),
            "mover saca el original: {}",
            op.origen.display()
        );
        let (tam, fecha, hash) = antes[&op.origen];
        let m = fs::metadata(&op.destino).unwrap();
        assert_eq!(m.len(), tam);
        assert_eq!(
            m.modified().unwrap(),
            fecha,
            "conserva la fecha: {}",
            op.destino.display()
        );
        assert_eq!(blake3::hash(&fs::read(&op.destino).unwrap()), hash);
    }
    assert!(plan.registro.exists(), "queda el registro para deshacer");
}

#[test]
fn copiar_deja_los_originales_y_copias_identicas() {
    let (dir, e) = escanear_demo();
    let antes = foto(&e.raiz);
    let destino = dir.path().join("Copia");
    let plan = ordenar(&e, &destino, Modo::Copiar, false);
    let r = acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    assert!(r.fallidos.is_empty(), "{:?}", r.fallidos);
    assert_eq!(antes, foto(&e.raiz), "los originales no se tocan");
    for op in &plan.operaciones {
        let (tam, fecha, hash) = antes[&op.origen];
        let m = fs::metadata(&op.destino).unwrap();
        assert_eq!((m.len(), m.modified().unwrap()), (tam, fecha));
        assert_eq!(blake3::hash(&fs::read(&op.destino).unwrap()), hash);
    }
    assert!(parciales(&destino).is_empty());
}

#[test]
fn nunca_pisa_un_archivo_que_ya_estaba() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Ordenado");
    let ocupado = destino.join("Fotos/2019/IMG_0001.jpg");
    fs::create_dir_all(ocupado.parent().unwrap()).unwrap();
    fs::write(&ocupado, "estaba antes").unwrap();

    let plan = ordenar(&e, &destino, Modo::Mover, true);
    acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    assert_eq!(fs::read_to_string(&ocupado).unwrap(), "estaba antes");
    assert!(destino.join("Fotos/2019/IMG_0001 (2).jpg").exists());
}

#[test]
fn si_aparece_un_archivo_despues_de_planificar_tampoco_lo_pisa() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Copiar, true);
    let ocupado = destino.join("Documentos/CV.pdf");
    fs::create_dir_all(ocupado.parent().unwrap()).unwrap();
    fs::write(&ocupado, "apareció").unwrap();

    let r = acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    assert_eq!(fs::read_to_string(&ocupado).unwrap(), "apareció");
    assert!(destino.join("Documentos/CV (2).pdf").exists());
    assert_eq!(r.renombrados, 1);
}

#[test]
fn si_un_archivo_cambio_desde_el_escaneo_no_se_mueve() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    let cambiado = e.raiz.join("Documentos/notas.txt");
    OpenOptions::new()
        .append(true)
        .open(&cambiado)
        .unwrap()
        .write_all(b"!")
        .unwrap();

    let r = acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    assert_eq!(r.fallidos.len(), 1);
    assert!(r.fallidos[0].1.contains("cambió"));
    assert!(cambiado.exists(), "el original queda donde estaba");
    assert_eq!(r.hechos, plan.operaciones.len() - 1, "los demás siguen");
}

#[test]
fn cancelar_no_deja_nada_a_medias() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Copia");
    let plan = ordenar(&e, &destino, Modo::Copiar, false);
    let p = ProgresoAccion::default();
    p.cancelar.store(true, Ordering::Relaxed);
    let r = acciones::ejecutar(&plan, &p).unwrap();
    assert!(r.cancelado);
    assert_eq!(r.hechos, 0);
    assert!(parciales(dir.path()).is_empty());
}

#[test]
fn el_destino_puede_estar_dentro_de_lo_escaneado() {
    let (_dir, e) = escanear_demo();
    let destino = e.raiz.join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();

    // Al volver a escanear, lo ya ordenado no se vuelve a mover y el registro no se cuenta.
    let e2 = reescanear(&e.raiz);
    assert!(e2.excluidos.iter().any(|x| x.ruta.ends_with("_analizador")));
    let plan2 = ordenar(&e2, &destino, Modo::Mover, true);
    assert_eq!(plan2.omitidos_en_destino, plan.operaciones.len());
}

// ─────────────────────── _para_borrar y deshacer ───────────────────────

#[test]
fn para_borrar_y_deshacer_deja_todo_como_estaba() {
    let (_dir, e) = escanear_demo();
    let antes = foto(&e.raiz);
    let marcados: Vec<Elemento> = borrable::por_archivo(&e, None)
        .iter()
        .enumerate()
        .filter(|(_, m)| m.is_some())
        .map(|(i, _)| Elemento::de(&e, i as u32))
        .collect();
    assert!(!marcados.is_empty());

    let plan = acciones::planificar_para_borrar(&marcados, &e.raiz);
    assert_eq!(plan.tipo, Tipo::ParaBorrar);
    acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    for op in &plan.operaciones {
        assert!(!op.origen.exists());
        let rel = op.origen.strip_prefix(&e.raiz).unwrap();
        assert_eq!(
            op.destino,
            e.raiz.join("_para_borrar").join(rel),
            "conserva la ruta original"
        );
        assert!(op.destino.exists());
    }

    // Lo que está en _para_borrar no se vuelve a escanear.
    let e2 = reescanear(&e.raiz);
    assert!(
        e2.excluidos
            .iter()
            .any(|x| x.ruta.ends_with("_para_borrar"))
    );

    let deshacer = acciones::planificar_deshacer(&e.raiz).unwrap();
    assert_eq!(deshacer.operaciones.len(), plan.operaciones.len());
    let r = acciones::ejecutar(&deshacer, &ProgresoAccion::default()).unwrap();
    assert!(r.fallidos.is_empty(), "{:?}", r.fallidos);

    let mut despues = foto(&e.raiz);
    despues.retain(|p, _| !p.starts_with(e.raiz.join("_para_borrar")));
    assert_eq!(
        antes, despues,
        "mismas rutas, mismo contenido, mismas fechas"
    );

    // El manifiesto queda sin lo deshecho: deshacer otra vez no hace nada.
    let otra_vez = acciones::planificar_deshacer(&e.raiz).unwrap();
    assert!(otra_vez.operaciones.is_empty());
}

#[test]
fn deshacer_un_ordenar() {
    let (dir, e) = escanear_demo();
    let antes = foto(&e.raiz);
    let destino = dir.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();

    let deshacer = acciones::planificar_deshacer(&plan.registro).unwrap();
    acciones::ejecutar(&deshacer, &ProgresoAccion::default()).unwrap();
    assert_eq!(antes, foto(&e.raiz));
}

#[test]
fn deshacer_no_pisa_lo_que_aparecio_en_el_lugar_original() {
    let (_dir, e) = escanear_demo();
    let notas = e.raiz.join("Documentos/notas.txt");
    let plan = acciones::planificar_para_borrar(
        &[Elemento::de(
            &e,
            e.archivos.iter().position(|a| a.ruta == notas).unwrap() as u32,
        )],
        &e.raiz,
    );
    acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    fs::write(&notas, "otras notas nuevas").unwrap();

    let deshacer = acciones::planificar_deshacer(&e.raiz).unwrap();
    assert_eq!(deshacer.renombrados, 1);
    acciones::ejecutar(&deshacer, &ProgresoAccion::default()).unwrap();
    assert_eq!(fs::read_to_string(&notas).unwrap(), "otras notas nuevas");
    assert!(e.raiz.join("Documentos/notas (2).txt").exists());
}

#[test]
fn deshacer_copias_no_borra_nada() {
    let (dir, e) = escanear_demo();
    let destino = dir.path().join("Copia");
    let plan = ordenar(&e, &destino, Modo::Copiar, false);
    acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    let deshacer = acciones::planificar_deshacer(&plan.registro).unwrap();
    assert!(
        deshacer.operaciones.is_empty(),
        "una copia no se deshace borrando"
    );
}

#[test]
fn deshacer_sin_registro_explica_que_pasar() {
    let dir = tempfile::tempdir().unwrap();
    let err = acciones::planificar_deshacer(dir.path()).unwrap_err();
    assert!(err.to_string().contains("manifiesto.csv"), "{err}");
}

#[test]
fn mover_entre_discos_copia_verifica_y_recien_ahi_borra() {
    // La demo queda en la carpeta temporal y el destino en `target/` del proyecto. Si están
    // en discos distintos (acá: C: y D:), mover es copiar + verificar + borrar el original.
    let (_dir, e) = escanear_demo();
    let otro = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    if acciones::mismo_volumen(&e.raiz, otro.path()) {
        eprintln!("(todo en el mismo disco en esta máquina: no se puede probar entre discos)");
        return;
    }
    let antes = foto(&e.raiz);
    let destino = otro.path().join("Ordenado");
    let plan = ordenar(&e, &destino, Modo::Mover, true);
    assert_eq!(
        plan.bytes_a_escribir, plan.bytes,
        "entre discos se copia todo"
    );
    let r = acciones::ejecutar(&plan, &ProgresoAccion::default()).unwrap();
    assert!(r.fallidos.is_empty(), "{:?}", r.fallidos);
    for op in &plan.operaciones {
        assert!(
            !op.origen.exists(),
            "el original se borra después de verificar"
        );
        let (tam, fecha, hash) = antes[&op.origen];
        let m = fs::metadata(&op.destino).unwrap();
        assert_eq!((m.len(), m.modified().unwrap()), (tam, fecha));
        assert_eq!(blake3::hash(&fs::read(&op.destino).unwrap()), hash);
    }
    assert!(parciales(&destino).is_empty());

    // Y se puede deshacer también entre discos.
    let deshacer = acciones::planificar_deshacer(&plan.registro).unwrap();
    acciones::ejecutar(&deshacer, &ProgresoAccion::default()).unwrap();
    assert_eq!(antes, foto(&e.raiz));
}

// ─────────────────────────── en la interfaz ───────────────────────────

mod interfaz {
    use super::*;
    use analizador_de_disco::app::{Accion, App, Entrada, Panel};
    use analizador_de_disco::iconos::Iconos;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn tecla(app: &mut App, code: KeyCode) {
        app.tecla(KeyEvent::from(code));
    }

    fn escribir(app: &mut App, texto: &str) {
        for c in texto.chars() {
            tecla(app, KeyCode::Char(c));
        }
    }

    fn esperar(app: &mut App) {
        let inicio = Instant::now();
        while app.ocupado() {
            assert!(inicio.elapsed() < Duration::from_secs(30), "no termina");
            std::thread::sleep(Duration::from_millis(5));
            app.tick();
        }
    }

    fn ir_a_carpeta(app: &mut App, raiz: &Path, relativa: &str) {
        let id = app.arbol.buscar(&raiz.join(relativa)).unwrap();
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

    #[test]
    fn espacio_marca_y_desmarca_una_carpeta_entera() {
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        let descargas = app.arbol.buscar(&e.raiz.join("Descargas")).unwrap();
        tecla(&mut app, KeyCode::Char(' '));
        assert_eq!(app.marcados.len(), 8);
        assert_eq!(app.marcados_en(descargas), 8);
        assert_eq!(app.marcados_en(analizador_de_disco::arbol::Arbol::RAIZ), 8);
        tecla(&mut app, KeyCode::Char(' '));
        assert!(app.marcados.is_empty());
        assert_eq!(app.marcados_bytes, 0);
        assert_eq!(app.marcados_en(descargas), 0);
    }

    #[test]
    fn espacio_en_una_categoria_marca_esa_categoria_de_la_carpeta() {
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        // Foco en Descargas; a la derecha, Instaladores.
        tecla(&mut app, KeyCode::Tab);
        for _ in 0..analizador_de_disco::categoria::Categoria::Instaladores as usize {
            tecla(&mut app, KeyCode::Down);
        }
        tecla(&mut app, KeyCode::Char(' '));
        assert_eq!(app.marcados.len(), 3, "setup.exe, ubuntu.iso y driver.msi");
    }

    #[test]
    fn ordenar_lo_marcado_desde_la_interfaz() {
        let (dir, e) = escanear_demo();
        let destino = dir.path().join("Ordenado");
        let mut app = App::new(&e, Iconos::Simples);
        ir_a_carpeta(&mut app, &e.raiz, "Documentos");
        tecla(&mut app, KeyCode::Char(' '));
        let marcados = app.marcados.len();

        tecla(&mut app, KeyCode::Char('c'));
        assert!(matches!(app.accion, Some(Accion::Destino { .. })));
        escribir(&mut app, &destino.display().to_string());
        tecla(&mut app, KeyCode::Enter);
        esperar(&mut app);
        let Some(Accion::Confirmar(plan)) = &app.accion else {
            panic!("tendría que mostrar la vista previa");
        };
        assert!(plan.se_puede_ejecutar());
        assert!(!destino.exists(), "la vista previa no hace nada");
        let n = plan.operaciones.len();
        assert_eq!(
            n + plan.omitidos_temporales,
            marcados,
            "el ~$ de Office queda afuera"
        );

        tecla(&mut app, KeyCode::Enter);
        esperar(&mut app);
        let Some(Accion::Listo { resultado, .. }) = &app.accion else {
            panic!("tendría que haber terminado");
        };
        assert_eq!(resultado.hechos, n);
        assert!(destino.join("Documentos/CV.pdf").exists());
        assert!(
            !e.raiz.join("Documentos/CV.pdf").exists(),
            "por defecto se mueve"
        );

        tecla(&mut app, KeyCode::Char('x'));
        assert!(
            app.salir && app.reescanear,
            "después de mover se vuelve a escanear"
        );
    }

    #[test]
    fn tab_en_el_destino_cambia_a_copiar() {
        let (dir, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        tecla(&mut app, KeyCode::Char(' '));
        tecla(&mut app, KeyCode::Char('c'));
        tecla(&mut app, KeyCode::Tab);
        escribir(&mut app, &dir.path().join("Copia").display().to_string());
        tecla(&mut app, KeyCode::Enter);
        esperar(&mut app);
        let Some(Accion::Confirmar(plan)) = &app.accion else {
            panic!("tendría que mostrar la vista previa");
        };
        assert_eq!(plan.tipo, Tipo::Ordenar(Modo::Copiar));
    }

    #[test]
    fn mandar_a_para_borrar_desde_la_pestana_borrable() {
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        tecla(&mut app, KeyCode::Char('4'));
        tecla(&mut app, KeyCode::Tab);
        tecla(&mut app, KeyCode::Char(' ')); // ubuntu.iso, el más pesado de Descargas
        tecla(&mut app, KeyCode::Char('x'));
        esperar(&mut app);
        let Some(Accion::Confirmar(plan)) = &app.accion else {
            panic!("tendría que mostrar la vista previa");
        };
        assert_eq!(plan.tipo, Tipo::ParaBorrar);
        assert_eq!(plan.operaciones.len(), 1);
        tecla(&mut app, KeyCode::Char('s'));
        esperar(&mut app);
        assert!(e.raiz.join("_para_borrar/Descargas/ubuntu.iso").exists());
    }

    #[test]
    fn cancelar_la_vista_previa_no_hace_nada() {
        let (dir, e) = escanear_demo();
        let antes = foto(&e.raiz);
        let mut app = App::new(&e, Iconos::Simples);
        tecla(&mut app, KeyCode::Char(' '));
        tecla(&mut app, KeyCode::Char('c'));
        escribir(&mut app, &dir.path().join("Ordenado").display().to_string());
        tecla(&mut app, KeyCode::Enter);
        esperar(&mut app);
        tecla(&mut app, KeyCode::Char('n'));
        assert!(app.accion.is_none());
        assert_eq!(antes, foto(&e.raiz));
        assert!(!dir.path().join("Ordenado").exists());
    }

    #[test]
    fn sin_nada_marcado_avisa_en_vez_de_abrir() {
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        tecla(&mut app, KeyCode::Char('c'));
        assert!(app.accion.is_none());
        assert!(app.mensaje.as_deref().unwrap().contains("Espacio"));
        tecla(&mut app, KeyCode::Down);
        assert!(
            app.mensaje.is_none(),
            "el mensaje se va con la próxima tecla"
        );
    }

    #[test]
    fn ctrl_c_mientras_se_mueve_cancela_en_vez_de_salir() {
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        let progreso = Arc::new(ProgresoAccion::default());
        let (_tx, rx) = std::sync::mpsc::channel();
        let plan = acciones::planificar_para_borrar(&[], &e.raiz);
        app.accion = Some(Accion::Ejecutando {
            plan,
            progreso: Arc::clone(&progreso),
            rx,
        });
        app.tecla(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.salir);
        assert!(progreso.cancelar.load(Ordering::Relaxed));
    }

    #[test]
    fn las_marcas_se_ven_en_pantalla() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        tecla(&mut app, KeyCode::Char(' ')); // Descargas entera
        tecla(&mut app, KeyCode::Enter);
        tecla(&mut app, KeyCode::Down);
        tecla(&mut app, KeyCode::Char(' ')); // desmarca un archivo: Descargas queda en parte
        tecla(&mut app, KeyCode::Backspace);
        let mut t = Terminal::new(TestBackend::new(140, 30)).unwrap();
        t.draw(|f| analizador_de_disco::ui::dibujar(f, &mut app))
            .unwrap();
        let buf = t.backend().buffer();
        let texto: String = (0..30)
            .map(|y| (0..140).map(|x| buf[(x, y)].symbol()).collect::<String>() + "\n")
            .collect();
        assert!(texto.contains("~ ► Descargas"), "{texto}");
        assert!(texto.contains("marcados 7"), "{texto}");
        assert!(texto.contains("c ordenar"), "{texto}");
        assert_eq!(app.panel, Panel::Disco);
    }
}

mod pantalla_accion {
    use super::*;
    use analizador_de_disco::app::{Accion, App};
    use analizador_de_disco::iconos::Iconos;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn pantalla(app: &mut App, ancho: u16, alto: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(ancho, alto)).unwrap();
        t.draw(|f| analizador_de_disco::ui::dibujar(f, app))
            .unwrap();
        let buf = t.backend().buffer();
        (0..alto)
            .map(|y| (0..ancho).map(|x| buf[(x, y)].symbol()).collect::<String>() + "\n")
            .collect()
    }

    /// Regresión: con líneas largas que se ajustan, la vista previa cortaba justo la
    /// última línea, la que dice cómo confirmar.
    #[test]
    fn la_vista_previa_siempre_muestra_como_confirmar() {
        let (dir, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        let plan = ordenar(&e, &dir.path().join("Ordenado"), Modo::Mover, false);
        app.accion = Some(Accion::Confirmar(plan));
        for (ancho, alto) in [(80, 24), (100, 30), (140, 40)] {
            let p = pantalla(&mut app, ancho, alto);
            assert!(p.contains("Enter/s: hacerlo"), "{ancho}x{alto}:\n{p}");
            assert!(p.contains("Vista previa"), "{ancho}x{alto}:\n{p}");
        }
    }

    #[test]
    fn un_plan_imposible_no_ofrece_hacerlo() {
        let (_d, e) = escanear_demo();
        let mut app = App::new(&e, Iconos::Simples);
        let plan = ordenar(&e, destino_de_sistema(), Modo::Mover, false);
        app.accion = Some(Accion::Confirmar(plan));
        let p = pantalla(&mut app, 120, 30);
        assert!(p.contains("No se puede"), "{p}");
        assert!(!p.contains("Enter/s: hacerlo"), "{p}");
        assert!(p.contains("Esc: volver"), "{p}");
    }
}
