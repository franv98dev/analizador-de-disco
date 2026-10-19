//! Cuándo se modificó cada cosa por última vez, agrupado por año.

use std::collections::BTreeMap;
use std::time::SystemTime;

use chrono::Datelike;

use crate::arbol::Suma;
use crate::escaner::Archivo;

/// Año local de una fecha.
pub fn anio(t: Option<SystemTime>) -> Option<i32> {
    t.map(|t| chrono::DateTime::<chrono::Local>::from(t).year())
}

/// De más viejo a más nuevo; los que no tienen fecha van al final.
pub fn por_anio<'a>(archivos: impl IntoIterator<Item = &'a Archivo>) -> Vec<(Option<i32>, Suma)> {
    let mut m: BTreeMap<Option<i32>, Suma> = BTreeMap::new();
    for a in archivos {
        *m.entry(anio(a.modificado)).or_default() += Suma {
            cantidad: 1,
            bytes: a.tamano,
        };
    }
    let mut v: Vec<(Option<i32>, Suma)> = m.into_iter().collect();
    // En un BTreeMap `None` va primero; para leer conviene al final.
    if v.first().is_some_and(|(a, _)| a.is_none()) {
        let sin_fecha = v.remove(0);
        v.push(sin_fecha);
    }
    v
}

pub fn nombre(anio: Option<i32>) -> String {
    anio.map_or_else(|| "sin fecha".to_string(), |a| a.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categoria::Categoria;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn archivo(anio: Option<i32>, tamano: u64) -> Archivo {
        Archivo {
            ruta: PathBuf::from("a"),
            tamano,
            modificado: anio.map(|a| {
                chrono::Local
                    .with_ymd_and_hms(a, 6, 15, 12, 0, 0)
                    .unwrap()
                    .into()
            }),
            categoria: Categoria::Otros,
            revisar: None,
            proyecto: None,
        }
    }

    #[test]
    fn agrupa_por_anio_de_viejo_a_nuevo_y_sin_fecha_al_final() {
        let v = [
            archivo(Some(2020), 10),
            archivo(None, 1),
            archivo(Some(2009), 5),
            archivo(Some(2020), 20),
        ];
        let r = por_anio(&v);
        assert_eq!(
            r,
            vec![
                (
                    Some(2009),
                    Suma {
                        cantidad: 1,
                        bytes: 5
                    }
                ),
                (
                    Some(2020),
                    Suma {
                        cantidad: 2,
                        bytes: 30
                    }
                ),
                (
                    None,
                    Suma {
                        cantidad: 1,
                        bytes: 1
                    }
                ),
            ]
        );
    }

    #[test]
    fn nombres() {
        assert_eq!(nombre(Some(2019)), "2019");
        assert_eq!(nombre(None), "sin fecha");
    }
}
