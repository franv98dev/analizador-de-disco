//! Escanea la carpeta de demo y verifica clasificación, exclusiones, "revisar",
//! proyectos y que el escaneo no modifique nada.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::time::SystemTime;

use analizador_de_disco::categoria::Categoria;
use analizador_de_disco::demo;
use analizador_de_disco::escaner::{Escaneo, Opciones, escanear};
use analizador_de_disco::resumen::Resumen;

fn escanear_demo(hilos: Option<usize>) -> (tempfile::TempDir, Escaneo) {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("demo");
    demo::crear(&raiz).unwrap();
    let escaneo = escanear(&raiz, &Opciones { hilos }, &AtomicU64::new(0)).unwrap();
    (dir, escaneo)
}

fn relativas(e: &Escaneo, rutas: impl Iterator<Item = PathBuf>) -> BTreeSet<String> {
    rutas
        .map(|r| {
            r.strip_prefix(&e.raiz)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

#[test]
fn cuenta_cada_categoria() {
    let (_dir, e) = escanear_demo(None);
    let r = Resumen::de(&e);
    let esperado = [
        (Categoria::Fotos, 6),  // 4 + una copia + un falso duplicado
        (Categoria::Videos, 3), // 2 + una copia
        (Categoria::Musica, 2),
        (Categoria::Grabaciones, 2),
        (Categoria::Documentos, 8), // 4 + LEEME + 2 copias del CV + ~$ de Office
        (Categoria::Proyectos, 6),  // 5 + node_modules
        (Categoria::Comprimidos, 2),
        (Categoria::Instaladores, 4), // 3 + un portable
        (Categoria::Otros, 5),        // 2 + .crdownload, .tmp y Thumbs.db
    ];
    for (c, n) in esperado {
        assert_eq!(r.categoria(c).cantidad, n, "cantidad de {}", c.nombre());
    }
    assert_eq!(
        r.categoria(Categoria::Fotos).bytes,
        (300 + 250 + 120 + 400 + 300 + 300) * 1024
    );
}

#[test]
fn excluye_el_sistema_y_no_lo_cuenta() {
    let (_dir, e) = escanear_demo(None);
    let excluidos = relativas(&e, e.excluidos.iter().map(|x| x.ruta.clone()));
    let esperado: BTreeSet<String> = [
        "$Recycle.Bin",
        "Program Files",
        "ProgramData",
        "System Volume Information",
        "Windows",
        "pagefile.sys",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(excluidos, esperado);

    let archivos = relativas(&e, e.archivos.iter().map(|a| a.ruta.clone()));
    assert!(
        !archivos
            .iter()
            .any(|a| a.starts_with("Windows/") || a.starts_with("Program Files/"))
    );
}

#[test]
fn una_carpeta_windows_del_usuario_no_se_excluye() {
    let (_dir, e) = escanear_demo(None);
    let a = e
        .archivos
        .iter()
        .find(|a| a.ruta.ends_with("trucos de windows.docx"))
        .expect("el docx dentro de Documentos/Windows tiene que aparecer");
    assert_eq!(a.categoria, Categoria::Documentos);
}

#[test]
fn appdata_va_a_revisar_agrupado_por_programa() {
    let (_dir, e) = escanear_demo(None);
    let r = Resumen::de(&e);
    let grupos = relativas(&e, r.revisar.iter().map(|g| g.ruta.to_path_buf()));
    let esperado: BTreeSet<String> = [
        "AppData/Local/Google",
        "AppData/Local/Temp",
        "AppData/LocalLow/Juego",
        "AppData/Roaming/WhatsApp",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(grupos, esperado);
    assert_eq!(r.revisar_total.cantidad, 4);
    // Lo de AppData no se mezcla con las categorías del usuario.
    assert_eq!(
        r.usuario.cantidad + r.revisar_total.cantidad,
        e.archivos.len() as u64
    );
}

#[test]
fn detecta_proyectos_y_todo_su_contenido() {
    let (_dir, e) = escanear_demo(None);
    let r = Resumen::de(&e);
    let proyectos = relativas(&e, r.proyectos.iter().map(|g| g.ruta.to_path_buf()));
    let esperado: BTreeSet<String> = ["Proyectos/juego", "Proyectos/mi-web"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(proyectos, esperado);

    let logo = e
        .archivos
        .iter()
        .find(|a| a.ruta.ends_with("logo.png"))
        .unwrap();
    assert_eq!(
        logo.categoria,
        Categoria::Proyectos,
        "una imagen dentro de un proyecto es del proyecto"
    );
}

#[test]
fn las_fechas_de_modificacion_se_leen() {
    let (_dir, e) = escanear_demo(None);
    let r = Resumen::de(&e);
    let docs = r.categoria(Categoria::Documentos);
    let anio = |t: SystemTime| {
        chrono::DateTime::<chrono::Utc>::from(t)
            .format("%Y")
            .to_string()
    };
    assert_eq!(anio(docs.mas_viejo.unwrap()), "2009");
}

#[test]
fn con_un_hilo_da_lo_mismo_que_en_paralelo() {
    let (_d1, uno) = escanear_demo(Some(1));
    let (_d2, varios) = escanear_demo(None);
    let a = relativas(&uno, uno.archivos.iter().map(|a| a.ruta.clone()));
    let b = relativas(&varios, varios.archivos.iter().map(|a| a.ruta.clone()));
    assert_eq!(a, b);
}

#[test]
fn el_escaneo_es_de_solo_lectura() {
    // Archivos: ruta, tamaño y fecha. Carpetas: solo que sigan estando. La fecha de una
    // carpeta que devuelve `read_dir` en NTFS es una copia del índice del padre que se
    // actualiza sola al cerrar un handle, así que no sirve para detectar escrituras.
    fn foto(raiz: &Path) -> BTreeSet<(PathBuf, Option<(u64, SystemTime)>)> {
        let mut v = BTreeSet::new();
        let mut pendientes = vec![raiz.to_path_buf()];
        while let Some(d) = pendientes.pop() {
            for e in fs::read_dir(&d).unwrap() {
                let ruta = e.unwrap().path();
                let m = fs::symlink_metadata(&ruta).unwrap();
                if m.is_dir() {
                    pendientes.push(ruta.clone());
                    v.insert((ruta, None));
                } else {
                    v.insert((ruta, Some((m.len(), m.modified().unwrap()))));
                }
            }
        }
        v
    }
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("demo");
    demo::crear(&raiz).unwrap();
    let antes = foto(&raiz);
    escanear(&raiz, &Opciones::default(), &AtomicU64::new(0)).unwrap();
    assert_eq!(antes, foto(&raiz));
}

#[test]
fn la_demo_no_pisa_una_carpeta_que_no_es_suya() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("importante.txt"), "no me borres").unwrap();
    assert!(demo::crear(dir.path()).is_err());
    assert!(dir.path().join("importante.txt").exists());
}

#[test]
fn la_demo_se_puede_recrear() {
    let dir = tempfile::tempdir().unwrap();
    let raiz = dir.path().join("demo");
    demo::crear(&raiz).unwrap();
    demo::crear(&raiz).unwrap();
}

#[test]
fn escanear_algo_que_no_existe_da_error() {
    assert!(
        escanear(
            Path::new("no/existe/esta/carpeta"),
            &Opciones::default(),
            &AtomicU64::new(0)
        )
        .is_err()
    );
}
