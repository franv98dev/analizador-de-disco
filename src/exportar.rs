//! Informe en Markdown (para leer) y CSV (para filtrar en Excel), con vista previa antes
//! de escribir. Nunca pisa un archivo existente salvo que se lo pida explícitamente.

use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use crate::categoria::Categoria;
use crate::escaner::{Archivo, Escaneo};
use crate::formato::{duracion, fecha, fecha_hora, miles, porcentaje, relativa, tamano};
use crate::resumen::{Grupo, Resumen, mas_pesados};

/// Cuántos archivos más pesados listar por categoría en el Markdown.
const TOP_MARKDOWN: usize = 20;
const MAX_ERRORES: usize = 50;

/// Lo que se va a escribir, para mostrarlo antes de confirmar.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub markdown: PathBuf,
    pub csv: PathBuf,
    /// Filas de datos del CSV (sin contar el encabezado).
    pub filas_csv: usize,
}

impl Plan {
    pub fn archivos(&self) -> [&Path; 2] {
        [&self.markdown, &self.csv]
    }

    /// Los que ya existen y se pisarían.
    pub fn existentes(&self) -> Vec<&Path> {
        self.archivos().into_iter().filter(|p| p.exists()).collect()
    }
}

/// Arma los nombres: `informe-<carpeta>-<fecha>_<hora>.md` y `.csv` dentro de `destino`.
pub fn planificar(destino: &Path, e: &Escaneo, ahora: DateTime<Local>) -> Plan {
    let base = format!(
        "informe-{}-{}",
        nombre_seguro(&e.raiz),
        ahora.format("%Y-%m-%d_%H%M")
    );
    Plan {
        markdown: destino.join(format!("{base}.md")),
        csv: destino.join(format!("{base}.csv")),
        filas_csv: e.archivos.len(),
    }
}

/// Escribe los dos archivos. Si `sobrescribir` es falso y alguno existe, falla sin tocar nada.
pub fn escribir(plan: &Plan, e: &Escaneo, sobrescribir: bool) -> io::Result<()> {
    if !sobrescribir && let Some(p) = plan.existentes().first() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} ya existe", p.display()),
        ));
    }
    if let Some(dir) = plan.markdown.parent() {
        fs::create_dir_all(dir)?;
    }
    let abrir = |ruta: &Path| {
        let mut o = OpenOptions::new();
        o.write(true);
        // `create_new` falla si el archivo apareció entre la vista previa y ahora.
        if sobrescribir {
            o.create(true).truncate(true);
        } else {
            o.create_new(true);
        }
        o.open(ruta).map(BufWriter::new)
    };
    let mut md = abrir(&plan.markdown)?;
    markdown(e, &mut md)?;
    md.flush()?;
    let mut csv_out = abrir(&plan.csv)?;
    csv(e, &mut csv_out)?;
    csv_out.flush()
}

/// CSV separado por `;` y en UTF-8 con BOM, para que Excel en español lo abra con doble clic.
pub fn csv(e: &Escaneo, out: &mut dyn Write) -> io::Result<()> {
    out.write_all("\u{feff}".as_bytes())?;
    out.write_all(b"ruta;categoria;zona;proyecto;carpeta_revisar;bytes;tamano;modificado\r\n")?;
    let mut archivos: Vec<&Archivo> = e.archivos.iter().collect();
    archivos.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    for a in archivos {
        let campos = [
            a.ruta.display().to_string(),
            a.categoria.nombre().to_string(),
            if a.revisar.is_some() {
                "revisar"
            } else {
                "usuario"
            }
            .to_string(),
            a.proyecto
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            a.revisar
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            a.tamano.to_string(),
            tamano(a.tamano),
            fecha_hora(a.modificado),
        ];
        let linea: Vec<String> = campos.iter().map(|c| campo_csv(c)).collect();
        out.write_all(linea.join(";").as_bytes())?;
        out.write_all(b"\r\n")?;
    }
    Ok(())
}

/// Entre comillas si tiene `;`, comillas o saltos de línea; las comillas se duplican.
pub fn campo_csv(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn markdown(e: &Escaneo, out: &mut dyn Write) -> io::Result<()> {
    let r = Resumen::de(e);
    writeln!(
        out,
        "# Informe de disco: {}",
        codigo(&e.raiz.display().to_string())
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "Generado el {}. Escaneo de {} archivos en {} ({} carpetas, {} archivos/s).",
        Local::now().format("%Y-%m-%d %H:%M"),
        miles(e.archivos.len() as u64),
        duracion(e.duracion),
        miles(e.carpetas),
        miles(e.archivos_por_segundo().round() as u64),
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "El escaneo es de solo lectura. Este informe es para revisar con calma qué conservar."
    )?;

    writeln!(out, "\n## Por categoría\n")?;
    writeln!(
        out,
        "| Categoría | Archivos | Tamaño | % | Más viejo | Más nuevo |"
    )?;
    writeln!(out, "|---|---:|---:|---:|---|---|")?;
    for c in Categoria::TODAS {
        let t = r.categoria(c);
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            c.nombre(),
            miles(t.cantidad),
            tamano(t.bytes),
            porcentaje(t.bytes, r.usuario.bytes),
            fecha(t.mas_viejo),
            fecha(t.mas_nuevo),
        )?;
    }
    writeln!(
        out,
        "| **Total del usuario** | **{}** | **{}** | | | |",
        miles(r.usuario.cantidad),
        tamano(r.usuario.bytes)
    )?;

    writeln!(out, "\n## Más pesados por categoría\n")?;
    for c in Categoria::TODAS {
        if c == Categoria::Proyectos {
            continue;
        }
        let archivos = mas_pesados(e, c, TOP_MARKDOWN);
        if archivos.is_empty() {
            continue;
        }
        writeln!(out, "### {}\n", c.nombre())?;
        writeln!(out, "| Tamaño | Modificado | Archivo |")?;
        writeln!(out, "|---:|---|---|")?;
        for a in archivos {
            writeln!(
                out,
                "| {} | {} | {} |",
                tamano(a.tamano),
                fecha(a.modificado),
                codigo(&relativa(&e.raiz, &a.ruta))
            )?;
        }
        writeln!(out)?;
    }

    if !r.proyectos.is_empty() {
        writeln!(out, "## Proyectos de código ({})\n", r.proyectos.len())?;
        tabla_grupos(out, &e.raiz, &r.proyectos)?;
    }

    if !r.revisar.is_empty() {
        writeln!(
            out,
            "## Revisar: datos de programas ({} archivos, {})\n",
            miles(r.revisar_total.cantidad),
            tamano(r.revisar_total.bytes)
        )?;
        writeln!(
            out,
            "No es sistema: puede haber partidas guardadas, perfiles de navegador o WhatsApp de escritorio.\n"
        )?;
        tabla_grupos(out, &e.raiz, &r.revisar)?;
    }

    if !e.excluidos.is_empty() {
        writeln!(
            out,
            "## Excluido por ser del sistema ({})\n",
            e.excluidos.len()
        )?;
        writeln!(out, "| Ruta | Motivo |")?;
        writeln!(out, "|---|---|")?;
        for x in &e.excluidos {
            writeln!(
                out,
                "| {} | {} |",
                codigo(&relativa(&e.raiz, &x.ruta)),
                x.motivo
            )?;
        }
        writeln!(out)?;
    }

    if !e.errores.is_empty() {
        writeln!(out, "## No se pudo leer ({})\n", e.errores.len())?;
        for x in e.errores.iter().take(MAX_ERRORES) {
            writeln!(
                out,
                "- {}: {}",
                codigo(&x.ruta.display().to_string()),
                x.mensaje
            )?;
        }
        if e.errores.len() > MAX_ERRORES {
            writeln!(out, "- … y {} más", e.errores.len() - MAX_ERRORES)?;
        }
        writeln!(out)?;
    }
    Ok(())
}

fn tabla_grupos(out: &mut dyn Write, raiz: &Path, grupos: &[Grupo]) -> io::Result<()> {
    writeln!(out, "| Tamaño | Archivos | Última modificación | Carpeta |")?;
    writeln!(out, "|---:|---:|---|---|")?;
    for g in grupos {
        writeln!(
            out,
            "| {} | {} | {} | {} |",
            tamano(g.totales.bytes),
            miles(g.totales.cantidad),
            fecha(g.totales.mas_nuevo),
            codigo(&relativa(raiz, &g.ruta))
        )?;
    }
    writeln!(out)
}

/// Ruta como código en línea: así `_`, `*` o `\` no se interpretan como formato.
/// La `|` se escapa porque cortaría la celda de la tabla.
pub fn codigo(s: &str) -> String {
    let s = s.replace('|', "\\|");
    if s.contains('`') {
        format!("`` {s} ``")
    } else {
        format!("`{s}`")
    }
}

/// Nombre de la carpeta escaneada apto para un nombre de archivo (`C:\` → `C`).
fn nombre_seguro(raiz: &Path) -> String {
    // A mano y no con `file_name`, para que dé lo mismo con rutas de Windows en Linux y al revés.
    let texto = raiz.display().to_string();
    let nombre = texto
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let limpio: String = nombre
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let limpio = limpio.trim_matches('_').to_string();
    if limpio.is_empty() {
        "disco".to_string()
    } else {
        limpio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campos_csv_con_caracteres_especiales() {
        assert_eq!(campo_csv("C:\\fotos\\a.jpg"), "C:\\fotos\\a.jpg");
        assert_eq!(campo_csv("a;b.txt"), "\"a;b.txt\"");
        assert_eq!(campo_csv("el \"mejor\".doc"), "\"el \"\"mejor\"\".doc\"");
        assert_eq!(campo_csv("dos\nlíneas"), "\"dos\nlíneas\"");
        assert_eq!(campo_csv("canción ñandú.mp3"), "canción ñandú.mp3");
    }

    #[test]
    fn rutas_como_codigo_en_markdown() {
        assert_eq!(codigo(r"C:\mis_fotos\*.jpg"), r"`C:\mis_fotos\*.jpg`");
        assert_eq!(codigo("a|b"), r"`a\|b`");
        assert_eq!(codigo("raro`nombre"), "`` raro`nombre ``");
    }

    #[test]
    fn nombres_de_archivo_seguros() {
        assert_eq!(nombre_seguro(Path::new(r"C:\Users\franv")), "franv");
        assert_eq!(nombre_seguro(Path::new("/home/papá")), "papá");
        assert_eq!(
            nombre_seguro(Path::new("/media/Mi Pendrive")),
            "Mi_Pendrive"
        );
        assert_eq!(nombre_seguro(Path::new(r"C:\")), "C");
        assert_eq!(nombre_seguro(Path::new("/")), "disco");
    }
}
