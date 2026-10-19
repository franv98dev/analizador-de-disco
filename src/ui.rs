//! Dibujo de la interfaz de dos paneles. Solo lee el estado de `App`.

use std::path::Path;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap};

use crate::app::{App, Derecha, Entrada, Exportar, Panel, Pestana};
use crate::arbol::Tipo;
use crate::categoria::Categoria;
use crate::formato::{duracion, fecha, miles, porcentaje, relativa, tamano};

const ACENTO: Color = Color::Cyan;
const APAGADO: Color = Color::DarkGray;
const REVISAR: Color = Color::Yellow;
const PROYECTO: Color = Color::Green;
const BARRA_DISCO: usize = 10;
const BARRA_CLASIFICADO: usize = 8;

pub fn color(c: Categoria) -> Color {
    match c {
        Categoria::Fotos => Color::Magenta,
        Categoria::Videos => Color::Red,
        Categoria::Musica => Color::Cyan,
        Categoria::Grabaciones => Color::LightCyan,
        Categoria::Documentos => Color::Yellow,
        Categoria::Proyectos => Color::Green,
        Categoria::Comprimidos => Color::Blue,
        Categoria::Instaladores => Color::LightRed,
        Categoria::Otros => Color::Gray,
    }
}

pub fn dibujar(f: &mut Frame, app: &mut App) {
    let [arriba, medio, abajo] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(f.area());
    let [izq, der] =
        Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)]).areas(medio);

    encabezado(f, app, arriba);
    panel_disco(f, app, izq);
    panel_clasificado(f, app, der);
    pie(f, app, abajo);
    if app.exportar.is_some() {
        modal_exportar(f, app);
    }
}

/// Pantalla mientras se escanea.
pub fn dibujar_progreso(f: &mut Frame, ruta: &Path, archivos: u64, transcurrido: Duration) {
    let area = centrado(f.area(), 60, 7);
    let segundos = transcurrido.as_secs_f64();
    let por_segundo = if segundos > 0.0 {
        archivos as f64 / segundos
    } else {
        0.0
    };
    let giro = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let giro = giro[(transcurrido.as_millis() / 100) as usize % giro.len()];
    let texto = vec![
        Line::from(vec![
            Span::raw(format!("{giro} Escaneando ")),
            Span::raw(ruta.display().to_string()).bold(),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw(miles(archivos)).fg(ACENTO).bold(),
            Span::raw(" archivos  ·  "),
            Span::raw(duracion(transcurrido)),
            Span::raw("  ·  "),
            Span::raw(miles(por_segundo.round() as u64)).fg(ACENTO),
            Span::raw(" archivos/s"),
        ]),
        Line::raw(""),
        Line::raw("q: cancelar").fg(APAGADO),
    ];
    f.render_widget(
        Paragraph::new(texto).block(bloque(" analizador ", true)),
        area,
    );
}

/// Pantalla entre el fin del escaneo y la interfaz, mientras se arma el árbol.
pub fn dibujar_clasificando(f: &mut Frame, archivos: u64) {
    let area = centrado(f.area(), 60, 5);
    let texto = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("Clasificando "),
            Span::raw(miles(archivos)).fg(ACENTO).bold(),
            Span::raw(" archivos…"),
        ]),
    ];
    f.render_widget(
        Paragraph::new(texto).block(bloque(" analizador ", true)),
        area,
    );
}

fn encabezado(f: &mut Frame, app: &App, area: Rect) {
    let r = &app.resumen;
    let linea = Line::from(vec![
        Span::raw(" analizador ").bg(ACENTO).fg(Color::Black).bold(),
        Span::raw(format!(" {} ", app.escaneo.raiz.display())).bold(),
        Span::raw(" · usuario ").fg(APAGADO),
        Span::raw(tamano(r.usuario.bytes)),
        Span::raw(" · revisar ").fg(APAGADO),
        Span::raw(tamano(r.revisar_total.bytes)).fg(REVISAR),
        Span::raw(" · excluido ").fg(APAGADO),
        Span::raw(miles(app.escaneo.excluidos.len() as u64)),
    ]);
    f.render_widget(Paragraph::new(linea), area);
}

fn panel_disco(f: &mut Frame, app: &mut App, area: Rect) {
    let ic = app.iconos;
    let actual = app.arbol.nodo(app.actual);
    let total = actual.total().bytes;
    let mut filas = Vec::with_capacity(app.entradas.len());

    for e in &app.entradas {
        let fila = match *e {
            Entrada::Subir => Row::new([Cell::from(format!("{} ..", ic.subir()))]).fg(APAGADO),
            Entrada::Carpeta(id) => {
                let n = app.arbol.nodo(id);
                match n.tipo {
                    // El motivo completo está en la pestaña Excluido.
                    Tipo::Excluido(_) => Row::new([Cell::from(format!(
                        "{} {}  (sistema)",
                        ic.excluido(),
                        n.nombre
                    ))])
                    .fg(APAGADO),
                    tipo => {
                        let bytes = n.total().bytes;
                        let (icono, estilo, sufijo) = if tipo == Tipo::Proyecto {
                            (ic.proyecto(), Style::new().fg(PROYECTO), "  (proyecto)")
                        } else if n.es_solo_revisar() {
                            (ic.revisar(), Style::new().fg(REVISAR), "  (revisar)")
                        } else {
                            (ic.carpeta(), Style::new().bold(), "")
                        };
                        Row::new([
                            Cell::from(format!("{icono} {}{sufijo}", n.nombre)),
                            derecha(tamano(bytes)),
                            Cell::from(barra(bytes, total, BARRA_DISCO)).fg(ACENTO),
                            derecha(porcentaje(bytes, total)),
                        ])
                        .style(estilo)
                    }
                }
            }
            Entrada::Archivo(i) => {
                let a = app.archivo(i);
                let nombre = a.ruta.file_name().unwrap_or_default().to_string_lossy();
                let estilo = if a.revisar.is_some() {
                    Style::new().fg(REVISAR)
                } else {
                    Style::new()
                };
                Row::new([
                    Cell::from(Line::from(vec![
                        Span::raw(ic.categoria(a.categoria)).fg(color(a.categoria)),
                        Span::raw(format!(" {nombre}")),
                    ])),
                    derecha(tamano(a.tamano)),
                    Cell::from(barra(a.tamano, total, BARRA_DISCO)).fg(APAGADO),
                    derecha(porcentaje(a.tamano, total)),
                ])
                .style(estilo)
            }
        };
        filas.push(fila);
    }

    let enfocado = app.panel == Panel::Disco;
    let titulo = format!(
        " {} Disco · {} ",
        ic.carpeta(),
        app.arbol.ruta(app.actual).display()
    );
    let pie = format!(
        " {} · {} archivos ",
        tamano(total),
        miles(actual.total().cantidad)
    );
    let tabla = Table::new(
        filas,
        [
            Constraint::Fill(1),
            Constraint::Length(10),
            Constraint::Length(BARRA_DISCO as u16),
            Constraint::Length(6),
        ],
    )
    .block(bloque(&titulo, enfocado).title_bottom(Line::from(pie).right_aligned()))
    .row_highlight_style(resaltado(enfocado));
    f.render_stateful_widget(tabla, area, &mut app.tabla_izq);
}

fn panel_clasificado(f: &mut Frame, app: &mut App, area: Rect) {
    let enfocado = app.panel == Panel::Clasificado;
    let foco = app.arbol.nodo(app.foco);
    let titulo = format!(" Clasificado · {} ", foco.nombre);
    let b = bloque(&titulo, enfocado);
    let interior = b.inner(area);
    f.render_widget(b, area);

    let [pestanas, contenido, detalle] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(interior);

    let nombres = Pestana::TODAS
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{} {}", i + 1, p.nombre()));
    let elegida = Pestana::TODAS
        .iter()
        .position(|p| *p == app.pestana)
        .unwrap_or(0);
    f.render_widget(
        Tabs::new(nombres)
            .select(elegida)
            .style(Style::new().fg(APAGADO))
            .highlight_style(Style::new().fg(ACENTO).bold().underlined()),
        pestanas,
    );

    let base = app.arbol.ruta(app.foco);
    let ic = app.iconos;
    let (filas, anchos, vacio): (Vec<Row>, Vec<Constraint>, &str) = match &app.derecha {
        Derecha::Categorias => {
            let total = foco.usuario_total().bytes;
            let filas = Categoria::TODAS
                .iter()
                .map(|&c| {
                    let s = foco.categoria(c);
                    let fila = Row::new([
                        Cell::from(Line::from(vec![
                            Span::raw(ic.categoria(c)).fg(color(c)),
                            Span::raw(format!(" {}", c.nombre())),
                        ])),
                        derecha(tamano(s.bytes)),
                        derecha(miles(s.cantidad)),
                        Cell::from(barra(s.bytes, total, BARRA_CLASIFICADO)).fg(color(c)),
                        derecha(porcentaje(s.bytes, total)),
                    ]);
                    if s.cantidad == 0 {
                        fila.fg(APAGADO)
                    } else {
                        fila
                    }
                })
                .collect();
            let anchos = vec![
                // Lo justo para "Grabaciones de voz" y "Proyectos/código" sin cortar.
                Constraint::Min(20),
                Constraint::Length(9),
                Constraint::Length(7),
                Constraint::Length(BARRA_CLASIFICADO as u16),
                Constraint::Length(6),
            ];
            (filas, anchos, "")
        }
        Derecha::Archivos(_, v) => {
            let filas = v
                .iter()
                .map(|&i| {
                    let a = app.archivo(i);
                    Row::new([
                        derecha(tamano(a.tamano)),
                        Cell::from(fecha(a.modificado)).fg(APAGADO),
                        Cell::from(relativa(&base, &a.ruta)),
                    ])
                })
                .collect();
            let anchos = vec![
                Constraint::Length(10),
                Constraint::Length(11),
                Constraint::Fill(1),
            ];
            (filas, anchos, "No hay archivos de esta categoría acá.")
        }
        Derecha::Grupos(v) => {
            let estilo = if app.pestana == Pestana::Revisar {
                REVISAR
            } else {
                PROYECTO
            };
            let filas = v
                .iter()
                .map(|g| {
                    Row::new([
                        derecha(tamano(g.totales.bytes)),
                        derecha(format!("{} arch.", miles(g.totales.cantidad))).fg(APAGADO),
                        Cell::from(relativa(&base, &g.ruta)).fg(estilo),
                    ])
                })
                .collect();
            let anchos = vec![
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Fill(1),
            ];
            let vacio = if app.pestana == Pestana::Revisar {
                "No hay datos de programas para revisar acá."
            } else {
                "No hay proyectos de código acá."
            };
            (filas, anchos, vacio)
        }
        Derecha::Excluidos(v) => {
            let filas = v
                .iter()
                .map(|x| {
                    Row::new([
                        Cell::from(relativa(&base, &x.ruta)),
                        Cell::from(x.motivo).fg(APAGADO),
                    ])
                })
                .collect();
            let anchos = vec![Constraint::Fill(1), Constraint::Fill(1)];
            (filas, anchos, "No se excluyó nada acá.")
        }
    };

    if filas.is_empty() {
        f.render_widget(Paragraph::new(vacio).fg(APAGADO), contenido);
    } else {
        let tabla = Table::new(filas, anchos).row_highlight_style(resaltado(enfocado));
        f.render_stateful_widget(tabla, contenido, &mut app.tabla_der);
    }

    let lineas = match (&app.derecha, &app.detalle) {
        (Derecha::Categorias, Some(t)) if t.cantidad == 0 => vec![
            Line::raw("Nada de esta categoría acá.").fg(APAGADO),
            Line::from(vec![
                Span::raw("Además, para revisar: ").fg(APAGADO),
                Span::raw(tamano(foco.revisar.bytes)).fg(REVISAR),
            ]),
        ],
        (Derecha::Categorias, Some(t)) => vec![
            Line::from(vec![
                Span::raw("Más viejo ").fg(APAGADO),
                Span::raw(fecha(t.mas_viejo)),
                Span::raw("  ·  más nuevo ").fg(APAGADO),
                Span::raw(fecha(t.mas_nuevo)),
            ]),
            Line::from(vec![
                Span::raw("Además, para revisar: ").fg(APAGADO),
                Span::raw(tamano(foco.revisar.bytes)).fg(REVISAR),
            ]),
        ],
        (Derecha::Archivos(c, v), _) => vec![Line::from(vec![
            Span::raw(c.nombre()).fg(color(*c)).bold(),
            Span::raw(format!(
                ": los {} archivos más pesados",
                miles(v.len() as u64)
            ))
            .fg(APAGADO),
        ])],
        _ => vec![Line::raw("Enter: abrir la carpeta a la izquierda").fg(APAGADO)],
    };
    f.render_widget(Paragraph::new(lineas), detalle);
}

fn pie(f: &mut Frame, app: &App, area: Rect) {
    let e = app.escaneo;
    let mut spans = vec![
        Span::raw(format!(
            " {} · {} arch/s ",
            duracion(e.duracion),
            miles(e.archivos_por_segundo().round() as u64)
        ))
        .bg(APAGADO)
        .fg(Color::White),
        Span::raw(" "),
    ];
    let teclas: &[(&str, &str)] = match (app.panel, &app.derecha) {
        (Panel::Disco, _) => &[
            ("↑↓", "mover"),
            ("Enter", "entrar"),
            ("⌫", "subir"),
            ("Tab", "panel"),
            ("1-4", "vista"),
            ("e", "exportar"),
            ("q", "salir"),
        ],
        (Panel::Clasificado, Derecha::Archivos(..)) => &[
            ("↑↓", "mover"),
            ("Enter", "ir a la carpeta"),
            ("←", "volver"),
            ("Tab", "panel"),
            ("e", "exportar"),
            ("q", "salir"),
        ],
        (Panel::Clasificado, Derecha::Categorias) => &[
            ("↑↓", "mover"),
            ("Enter", "ver archivos"),
            ("←→", "vista"),
            ("Tab", "panel"),
            ("e", "exportar"),
            ("q", "salir"),
        ],
        (Panel::Clasificado, _) => &[
            ("↑↓", "mover"),
            ("Enter", "ir a la carpeta"),
            ("←→", "vista"),
            ("Tab", "panel"),
            ("e", "exportar"),
            ("q", "salir"),
        ],
    };
    for (tecla, que) in teclas {
        spans.push(Span::raw(*tecla).fg(ACENTO).bold());
        spans.push(Span::raw(format!(" {que}  ")).fg(APAGADO));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn modal_exportar(f: &mut Frame, app: &App) {
    let Some(estado) = &app.exportar else {
        return;
    };
    let area = centrado(f.area(), 76, 11);
    let lineas: Vec<Line> = match estado {
        Exportar::Destino(d) => vec![
            Line::raw("Carpeta donde guardar el informe (Markdown + CSV):"),
            Line::raw(""),
            Line::from(vec![
                Span::raw("> ").fg(ACENTO),
                Span::raw(d.as_str()),
                Span::raw("█").fg(ACENTO),
            ]),
            Line::raw(""),
            Line::raw("Enter: vista previa  ·  Esc: cancelar").fg(APAGADO),
        ],
        Exportar::Confirmar { plan, sobrescribe } => {
            let mut v = vec![
                Line::raw("Vista previa. Se van a crear:"),
                Line::raw(""),
                Line::from(vec![
                    Span::raw(plan.markdown.display().to_string()).fg(ACENTO),
                    Span::raw("  (resumen)").fg(APAGADO),
                ]),
                Line::from(vec![
                    Span::raw(plan.csv.display().to_string()).fg(ACENTO),
                    Span::raw(format!("  ({} filas)", miles(plan.filas_csv as u64))).fg(APAGADO),
                ]),
                Line::raw(""),
            ];
            if *sobrescribe {
                v.push(
                    Line::raw("Ojo: ya existen y se van a sobrescribir.")
                        .fg(Color::Red)
                        .bold(),
                );
            }
            v.push(Line::raw("Enter/s: exportar  ·  n/Esc: cancelar").fg(APAGADO));
            v
        }
        Exportar::Listo(msg) => {
            let mut v: Vec<Line> = msg
                .lines()
                .map(|l| Line::raw(l.to_string()).fg(Color::Green))
                .collect();
            v.push(Line::raw(""));
            v.push(Line::raw("Cualquier tecla para cerrar").fg(APAGADO));
            v
        }
        Exportar::Error(msg) => vec![
            Line::raw("No se pudo exportar:").fg(Color::Red).bold(),
            Line::raw(msg.as_str()),
            Line::raw(""),
            Line::raw("Cualquier tecla para cerrar").fg(APAGADO),
        ],
    };
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lineas)
            .wrap(Wrap { trim: false })
            .block(bloque(" Exportar informe ", true)),
        area,
    );
}

fn bloque(titulo: &str, enfocado: bool) -> Block<'static> {
    let borde = if enfocado {
        Style::new().fg(ACENTO)
    } else {
        Style::new().fg(APAGADO)
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(borde)
        .title(Line::raw(titulo.to_string()).bold())
}

fn resaltado(enfocado: bool) -> Style {
    if enfocado {
        Style::new()
            .bg(Color::Rgb(40, 60, 80))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::DIM)
    }
}

/// Celda alineada a la derecha, para números.
fn derecha(texto: String) -> Cell<'static> {
    Cell::from(Line::from(texto).right_aligned())
}

/// Barra proporcional con bloques llenos y vacíos.
pub fn barra(parte: u64, total: u64, ancho: usize) -> String {
    let frac = if total == 0 {
        0.0
    } else {
        (parte as f64 / total as f64).clamp(0.0, 1.0)
    };
    let llenos = (frac * ancho as f64).round() as usize;
    format!("{}{}", "█".repeat(llenos), "░".repeat(ancho - llenos))
}

fn centrado(area: Rect, ancho: u16, alto: u16) -> Rect {
    let [v] = Layout::vertical([Constraint::Length(alto)])
        .flex(Flex::Center)
        .areas(area);
    let [h] = Layout::horizontal([Constraint::Length(ancho)])
        .flex(Flex::Center)
        .areas(v);
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barras_proporcionales() {
        assert_eq!(barra(0, 100, 10), "░░░░░░░░░░");
        assert_eq!(barra(50, 100, 10), "█████░░░░░");
        assert_eq!(barra(100, 100, 10), "██████████");
        assert_eq!(barra(5, 0, 10), "░░░░░░░░░░");
        assert_eq!(barra(200, 100, 10), "██████████");
        assert_eq!(barra(1, 4, 8), "██░░░░░░");
    }
}
