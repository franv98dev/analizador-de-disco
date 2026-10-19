//! Informe de texto para la terminal.

use std::io::{self, Write};
use std::path::Path;

use crate::categoria::Categoria;
use crate::escaner::Escaneo;
use crate::formato::{duracion, fecha, miles, porcentaje, relativa, tamano};
use crate::resumen::{Grupo, Resumen, mas_pesados};

const LINEA: &str =
    "────────────────────────────────────────────────────────────────────────────────";
/// Cuántas carpetas de "revisar", proyectos y exclusiones listar como máximo.
const MAX_GRUPOS: usize = 15;

/// `top` = cuántos archivos más pesados mostrar por categoría.
pub fn imprimir(e: &Escaneo, top: usize, out: &mut dyn Write) -> io::Result<()> {
    let r = Resumen::de(e);

    writeln!(out, "\nEscaneo de {}", e.raiz.display())?;
    writeln!(out, "{LINEA}")?;
    writeln!(
        out,
        "{:<20} {:>10} {:>11} {:>7}   {:<10}   {:<10}",
        "Categoría", "Archivos", "Tamaño", "%", "Más viejo", "Más nuevo"
    )?;
    for c in Categoria::TODAS {
        let t = r.categoria(c);
        writeln!(
            out,
            "{:<20} {:>10} {:>11} {:>7}   {:<10}   {:<10}",
            c.nombre(),
            miles(t.cantidad),
            tamano(t.bytes),
            porcentaje(t.bytes, r.usuario.bytes),
            fecha(t.mas_viejo),
            fecha(t.mas_nuevo),
        )?;
    }
    writeln!(out, "{LINEA}")?;
    writeln!(
        out,
        "{:<20} {:>10} {:>11}",
        "Total del usuario",
        miles(r.usuario.cantidad),
        tamano(r.usuario.bytes)
    )?;

    if top > 0 {
        writeln!(out, "\nMás pesados por categoría")?;
        for c in Categoria::TODAS {
            if c == Categoria::Proyectos {
                continue; // los proyectos se listan enteros más abajo
            }
            let archivos = mas_pesados(e, c, top);
            if archivos.is_empty() {
                continue;
            }
            writeln!(out, "  {}", c.nombre())?;
            for a in archivos {
                writeln!(
                    out,
                    "    {:>11}  {}",
                    tamano(a.tamano),
                    relativa(&e.raiz, &a.ruta)
                )?;
            }
        }
    }

    if !r.proyectos.is_empty() {
        writeln!(out, "\nProyectos de código ({})", r.proyectos.len())?;
        grupos(out, &e.raiz, &r.proyectos)?;
    }

    if !r.revisar.is_empty() {
        writeln!(
            out,
            "\nRevisar: datos de programas que pueden ser valiosos ({} archivos, {})",
            miles(r.revisar_total.cantidad),
            tamano(r.revisar_total.bytes)
        )?;
        writeln!(
            out,
            "  Partidas guardadas, perfiles de navegador, WhatsApp de escritorio… No es sistema."
        )?;
        grupos(out, &e.raiz, &r.revisar)?;
    }

    if !e.excluidos.is_empty() {
        writeln!(
            out,
            "\nExcluido por ser del sistema ({})",
            e.excluidos.len()
        )?;
        for x in e.excluidos.iter().take(MAX_GRUPOS) {
            writeln!(out, "  {:<40} {}", relativa(&e.raiz, &x.ruta), x.motivo)?;
        }
        if e.excluidos.len() > MAX_GRUPOS {
            writeln!(out, "  … y {} más", e.excluidos.len() - MAX_GRUPOS)?;
        }
    }

    if !e.errores.is_empty() {
        writeln!(
            out,
            "\nNo se pudo leer ({}), probablemente por permisos:",
            e.errores.len()
        )?;
        for x in e.errores.iter().take(5) {
            writeln!(out, "  {}: {}", x.ruta.display(), x.mensaje)?;
        }
    }

    writeln!(out, "\n{LINEA}")?;
    writeln!(
        out,
        "Tiempo: {}  ·  {} archivos  ·  {} carpetas  ·  {} archivos/s",
        duracion(e.duracion),
        miles(e.archivos.len() as u64),
        miles(e.carpetas),
        miles(e.archivos_por_segundo().round() as u64),
    )?;
    if e.enlaces_omitidos > 0 {
        writeln!(
            out,
            "Enlaces/junctions no seguidos: {}",
            miles(e.enlaces_omitidos)
        )?;
    }
    Ok(())
}

fn grupos(out: &mut dyn Write, raiz: &Path, grupos: &[Grupo]) -> io::Result<()> {
    for g in grupos.iter().take(MAX_GRUPOS) {
        writeln!(
            out,
            "  {:>11}  {:>8} arch.  {}",
            tamano(g.totales.bytes),
            miles(g.totales.cantidad),
            relativa(raiz, &g.ruta)
        )?;
    }
    if grupos.len() > MAX_GRUPOS {
        writeln!(out, "  … y {} más", grupos.len() - MAX_GRUPOS)?;
    }
    Ok(())
}
