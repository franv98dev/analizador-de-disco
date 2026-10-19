//! Dibujo de la interfaz de dos paneles. Solo lee el estado de `App`.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap};

use crate::acciones::{self, Modo, Nivel, Tipo as TipoAccion};
use crate::antiguedad;
use crate::app::{Accion, App, Derecha, Entrada, Exportar, Filtro, Panel, Pestana};
use crate::arbol::Tipo;
use crate::borrable::{self, Motivo};
use crate::categoria::Categoria;
use crate::formato::{duracion, fecha, miles, porcentaje, relativa, tamano};

const ACENTO: Color = Color::Cyan;
const MARCADO: Color = Color::LightGreen;
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
    if app.accion.is_some() {
        modal_accion(f, app);
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
    let mut linea = linea;
    if !app.marcados.is_empty() {
        linea.push_span(Span::raw(" · marcados ").fg(APAGADO));
        linea.push_span(
            Span::raw(format!(
                "{} ({})",
                miles(app.marcados.len() as u64),
                tamano(app.marcados_bytes)
            ))
            .fg(MARCADO)
            .bold(),
        );
    }
    f.render_widget(Paragraph::new(linea), area);
}

fn panel_disco(f: &mut Frame, app: &mut App, area: Rect) {
    let ic = app.iconos;
    let actual = app.arbol.nodo(app.actual);
    let total = actual.total().bytes;
    let mut filas = Vec::with_capacity(app.entradas.len());

    for e in &app.entradas {
        let fila = match *e {
            Entrada::Subir => {
                Row::new([Cell::from(" "), Cell::from(format!("{} ..", ic.subir()))]).fg(APAGADO)
            }
            Entrada::Carpeta(id) => {
                let n = app.arbol.nodo(id);
                match n.tipo {
                    // El motivo completo está en la pestaña Excluido.
                    Tipo::Excluido(_) => Row::new([
                        Cell::from(" "),
                        Cell::from(format!("{} {}  (sistema)", ic.excluido(), n.nombre)),
                    ])
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
                            marca_carpeta(app, id),
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
                    marca_archivo(app, i),
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
            Constraint::Length(1),
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

    // Con 8 pestañas no entran todos los nombres: se ve el número de cada una
    // y el nombre de la elegida.
    let elegida = Pestana::TODAS
        .iter()
        .position(|p| *p == app.pestana)
        .unwrap_or(0);
    let titulos = Pestana::TODAS.iter().enumerate().map(|(i, p)| {
        if *p == app.pestana {
            format!("{} {}", i + 1, p.nombre())
        } else {
            (i + 1).to_string()
        }
    });
    f.render_widget(
        Tabs::new(titulos)
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
                    let ruta = Cell::from(relativa(&base, &a.ruta));
                    Row::new([
                        marca_archivo(app, i),
                        Cell::from(ic.categoria(a.categoria)).fg(color(a.categoria)),
                        derecha(tamano(a.tamano)),
                        Cell::from(fecha(a.modificado)).fg(APAGADO),
                        if a.revisar.is_some() {
                            ruta.fg(REVISAR)
                        } else {
                            ruta
                        },
                    ])
                })
                .collect();
            let anchos = vec![
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(9),
                Constraint::Length(10),
                Constraint::Fill(1),
            ];
            (filas, anchos, "No hay archivos acá.")
        }
        Derecha::Anios(v) => {
            let total: u64 = v.iter().map(|(_, s)| s.bytes).sum();
            let filas = v
                .iter()
                .map(|(anio, s)| {
                    Row::new([
                        Cell::from(antiguedad::nombre(*anio)),
                        derecha(tamano(s.bytes)),
                        derecha(miles(s.cantidad)),
                        Cell::from(barra(s.bytes, total, BARRA_CLASIFICADO)).fg(ACENTO),
                        derecha(porcentaje(s.bytes, total)),
                    ])
                })
                .collect();
            let anchos = vec![
                Constraint::Min(10),
                Constraint::Length(9),
                Constraint::Length(7),
                Constraint::Length(BARRA_CLASIFICADO as u16),
                Constraint::Length(6),
            ];
            (filas, anchos, "No hay archivos acá.")
        }
        Derecha::Borrable(v) => {
            let filas = v
                .iter()
                .map(|i| {
                    let cantidad = if i.archivo.is_some() {
                        String::new()
                    } else {
                        format!("{} arch.", miles(i.suma.cantidad))
                    };
                    Row::new([
                        Cell::from(i.motivo.nombre()).fg(color_motivo(i.motivo)),
                        derecha(tamano(i.suma.bytes)),
                        derecha(cantidad).fg(APAGADO),
                        Cell::from(relativa(&base, &i.ruta)),
                    ])
                })
                .collect();
            let anchos = vec![
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Length(11),
                Constraint::Fill(1),
            ];
            (filas, anchos, "Nada para sugerir acá.")
        }
        Derecha::Duplicados(v) => {
            let filas = v
                .iter()
                .filter_map(|&g| app.grupo(g))
                .map(|g| {
                    let primero = app.archivo(g.archivos[0]);
                    Row::new([
                        derecha(tamano(g.liberable())),
                        derecha(format!("×{}", g.archivos.len())).fg(APAGADO),
                        Cell::from(relativa(&base, &primero.ruta)),
                    ])
                })
                .collect();
            let anchos = vec![
                Constraint::Length(9),
                Constraint::Length(4),
                Constraint::Fill(1),
            ];
            let vacio = if app.busqueda.is_some() {
                "Buscando duplicados…"
            } else if app.duplicados.is_none() {
                "Todavía no se buscaron duplicados. Tocá d para buscar."
            } else {
                "No hay duplicados acá."
            };
            (filas, anchos, vacio)
        }
        Derecha::Copias(g) => {
            let filas = app
                .grupo(*g)
                .map(|g| {
                    g.archivos
                        .iter()
                        .enumerate()
                        .map(|(i, &archivo)| {
                            let a = app.archivo(archivo);
                            let marca = if i == 0 {
                                Cell::from("conservar").fg(Color::Green)
                            } else {
                                Cell::from("copia").fg(color_motivo(Motivo::Duplicado))
                            };
                            Row::new([
                                marca_archivo(app, archivo),
                                marca,
                                Cell::from(fecha(a.modificado)).fg(APAGADO),
                                Cell::from(relativa(&base, &a.ruta)),
                            ])
                        })
                        .collect()
                })
                .unwrap_or_default();
            let anchos = vec![
                Constraint::Length(1),
                Constraint::Length(9),
                Constraint::Length(10),
                Constraint::Fill(1),
            ];
            (filas, anchos, "")
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
        f.render_widget(
            Paragraph::new(vacio).fg(APAGADO).wrap(Wrap { trim: true }),
            contenido,
        );
    } else {
        let tabla = Table::new(filas, anchos).row_highlight_style(resaltado(enfocado));
        f.render_stateful_widget(tabla, contenido, &mut app.tabla_der);
    }

    f.render_widget(Paragraph::new(lineas_detalle(app)), detalle);
}

/// Las dos líneas de abajo del panel derecho: fechas, totales, avance o ayuda.
fn lineas_detalle(app: &App) -> Vec<Line<'static>> {
    let foco = app.arbol.nodo(app.foco);
    let ayuda = |t: &str| Line::raw(t.to_string()).fg(APAGADO);
    let revisar = Line::from(vec![
        Span::raw("Además, para revisar: ").fg(APAGADO),
        Span::raw(tamano(foco.revisar.bytes)).fg(REVISAR),
    ]);
    match &app.derecha {
        Derecha::Categorias => match &app.detalle {
            Some(t) if t.cantidad > 0 => vec![
                Line::from(vec![
                    Span::raw("Más viejo ").fg(APAGADO),
                    Span::raw(fecha(t.mas_viejo)),
                    Span::raw("  ·  más nuevo ").fg(APAGADO),
                    Span::raw(fecha(t.mas_nuevo)),
                ]),
                revisar,
            ],
            _ => vec![ayuda("Nada de esta categoría acá."), revisar],
        },
        Derecha::Archivos(filtro, v) => {
            let n = miles(v.len() as u64);
            let texto = match filtro {
                Some(Filtro::Categoria(c)) => {
                    return vec![Line::from(vec![
                        Span::raw(c.nombre()).fg(color(*c)).bold(),
                        Span::raw(format!(": los {n} archivos más pesados")).fg(APAGADO),
                    ])];
                }
                Some(Filtro::Anio(a)) => format!(
                    "Modificados en {}: los {n} más pesados",
                    antiguedad::nombre(*a)
                ),
                None => format!("Los {n} archivos más pesados, de cualquier tipo"),
            };
            vec![ayuda(&texto)]
        }
        Derecha::Anios(_) => vec![
            ayuda("Año de la última modificación (sin contar lo de revisar)."),
            ayuda("Enter: ver los archivos de ese año"),
        ],
        Derecha::Borrable(v) => {
            let totales = borrable::totales(v);
            let mut spans = Vec::new();
            let mut total = 0;
            for (m, s) in &totales {
                if !spans.is_empty() {
                    spans.push(Span::raw(" · ").fg(APAGADO));
                }
                spans.push(Span::raw(format!("{} ", m.nombre())).fg(color_motivo(*m)));
                spans.push(Span::raw(tamano(s.bytes)));
                total += s.bytes;
            }
            let mut segunda = vec![
                Span::raw("Total ").fg(APAGADO),
                Span::raw(tamano(total)).bold(),
                Span::raw(" · solo sugerencias").fg(APAGADO),
            ];
            if app.duplicados.is_none() {
                segunda.push(Span::raw(" · d: sumar copias").fg(APAGADO));
            }
            vec![Line::from(spans), Line::from(segunda)]
        }
        Derecha::Duplicados(v) => {
            if let Some(b) = &app.busqueda {
                let p = &b.progreso;
                let hechos = p.bytes_hechos.load(Ordering::Relaxed);
                let total = p.bytes_total.load(Ordering::Relaxed);
                let etapa = match p.etapa.load(Ordering::Relaxed) {
                    0 => "Preparando",
                    1 => "1/2 principio y final",
                    _ => "2/2 archivos completos",
                };
                return vec![
                    Line::from(vec![
                        Span::raw(format!("{etapa} · ")).fg(APAGADO),
                        Span::raw(porcentaje(hechos, total)).fg(ACENTO).bold(),
                        Span::raw(format!(" · {} de {}", tamano(hechos), tamano(total)))
                            .fg(APAGADO),
                    ]),
                    ayuda(&format!(
                        "{} candidatos ({}) · d: cancelar",
                        miles(b.candidatos as u64),
                        tamano(b.bytes)
                    )),
                ];
            }
            if let Some(aviso) = &app.aviso {
                return vec![
                    Line::raw(aviso.clone()).fg(Color::Red),
                    ayuda("d: buscar duplicados"),
                ];
            }
            match &app.duplicados {
                Some(d) => {
                    let liberable: u64 = v
                        .iter()
                        .filter_map(|&g| app.grupo(g))
                        .map(|g| g.liberable())
                        .sum();
                    vec![
                        Line::from(vec![
                            Span::raw(format!("{} grupos · se liberan ", miles(v.len() as u64)))
                                .fg(APAGADO),
                            Span::raw(tamano(liberable)).bold(),
                        ]),
                        ayuda(&format!(
                            "Leído en {} ({:.0} MB/s) · Enter: ver copias · d: buscar de nuevo",
                            duracion(d.duracion),
                            d.mb_por_segundo()
                        )),
                    ]
                }
                None => vec![
                    ayuda("Lee el contenido de los archivos que comparten tamaño."),
                    ayuda("d: buscar duplicados"),
                ],
            }
        }
        Derecha::Copias(g) => match app.grupo(*g) {
            Some(g) => vec![
                Line::from(vec![
                    Span::raw("Borrando las copias se liberan ").fg(APAGADO),
                    Span::raw(tamano(g.liberable())).bold(),
                ]),
                ayuda("Se sugiere conservar el más viejo · Enter: ir a la carpeta"),
            ],
            None => Vec::new(),
        },
        Derecha::Grupos(_) | Derecha::Excluidos(_) => {
            vec![ayuda("Enter: abrir la carpeta a la izquierda")]
        }
    }
}

/// `*` si la carpeta está marcada entera, `~` si en parte.
fn marca_carpeta(app: &App, id: usize) -> Cell<'static> {
    let marcados = app.marcados_en(id);
    if marcados == 0 {
        Cell::from(" ")
    } else if marcados >= app.arbol.nodo(id).total().cantidad {
        Cell::from("*").fg(MARCADO).bold()
    } else {
        Cell::from("~").fg(MARCADO)
    }
}

fn marca_archivo(app: &App, i: u32) -> Cell<'static> {
    if app.marcados.contains(&i) {
        Cell::from("*").fg(MARCADO).bold()
    } else {
        Cell::from(" ")
    }
}

fn color_motivo(m: Motivo) -> Color {
    match m {
        Motivo::Temporal => Color::LightRed,
        Motivo::Cache => Color::Blue,
        Motivo::Instalador => Color::Magenta,
        Motivo::Duplicado => Color::Yellow,
    }
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
    if let Some(m) = &app.mensaje {
        spans.push(Span::raw(m.clone()).fg(REVISAR).bold());
        f.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }
    let subvista = matches!(
        app.derecha,
        Derecha::Archivos(Some(_), _) | Derecha::Copias(_)
    );
    let navegar: &[(&str, &str)] = match (app.panel, &app.derecha) {
        (Panel::Disco, _) => &[("Enter", "entrar"), ("⌫", "subir")],
        (Panel::Clasificado, _) if subvista => &[("Enter", "ir"), ("←", "volver")],
        (Panel::Clasificado, Derecha::Categorias | Derecha::Anios(_)) => {
            &[("Enter", "ver"), ("←→", "vista")]
        }
        (Panel::Clasificado, Derecha::Duplicados(_)) => &[("Enter", "copias"), ("←→", "vista")],
        (Panel::Clasificado, _) => &[("Enter", "ir"), ("←→", "vista")],
    };
    let mut teclas: Vec<(&str, &str)> = navegar.to_vec();
    teclas.extend([("Tab", "panel"), ("1-8", "vista"), ("Espacio", "marcar")]);
    if app.marcados.is_empty() {
        teclas.extend([("d", "duplicados"), ("e", "informe")]);
    } else {
        // Con algo marcado importa más qué hacer con eso (y así entra en 120 columnas).
        teclas.extend([("c", "ordenar"), ("x", "a borrar"), ("u", "desmarcar")]);
    }
    teclas.push(("q", "salir"));
    for (tecla, que) in teclas {
        spans.push(Span::raw(tecla).fg(ACENTO).bold());
        spans.push(Span::raw(format!(" {que}  ")).fg(APAGADO));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn modal_accion(f: &mut Frame, app: &App) {
    let Some(accion) = &app.accion else {
        return;
    };
    let ayuda = |t: &str| Line::raw(t.to_string()).fg(APAGADO);
    let (titulo, lineas): (&str, Vec<Line>) = match accion {
        Accion::Destino { texto, modo } => {
            let (modo, otro) = match modo {
                Modo::Mover => ("mover", "copiar"),
                Modo::Copiar => ("copiar", "mover"),
            };
            (
                " Ordenar lo marcado ",
                vec![
                    Line::raw(format!(
                        "{} archivos marcados ({}). Carpeta de destino:",
                        miles(app.marcados.len() as u64),
                        tamano(app.marcados_bytes)
                    )),
                    Line::raw(""),
                    Line::from(vec![
                        Span::raw("> ").fg(ACENTO),
                        Span::raw(texto.clone()),
                        Span::raw("█").fg(ACENTO),
                    ]),
                    Line::raw(""),
                    Line::from(vec![
                        Span::raw("Modo: ").fg(APAGADO),
                        Span::raw(modo).bold(),
                        Span::raw(format!("  (Tab: {otro})")).fg(APAGADO),
                    ]),
                    ayuda("Fotos, videos y grabaciones por año; el resto con su carpeta original."),
                    Line::raw(""),
                    ayuda("Enter: vista previa (todavía no se hace nada)  ·  Esc: cancelar"),
                ],
            )
        }
        Accion::Preparando(_) => (
            " Preparando ",
            vec![
                Line::raw(""),
                Line::raw("Armando el plan (leyendo la fecha EXIF de las fotos)…"),
                Line::raw(""),
                ayuda("Esc: cancelar"),
            ],
        ),
        Accion::Confirmar(plan) => {
            let mut v: Vec<Line> = acciones::describir(plan, &app.escaneo.raiz, 5)
                .into_iter()
                .map(|(nivel, texto)| match nivel {
                    Nivel::Titulo => Line::raw(texto).bold(),
                    Nivel::Normal => Line::raw(texto),
                    Nivel::Aviso => Line::raw(texto).fg(REVISAR),
                    Nivel::Problema => Line::raw(texto).fg(Color::Red).bold(),
                })
                .collect();
            v.push(Line::raw(""));
            if plan.se_puede_ejecutar() {
                if matches!(
                    plan.tipo,
                    TipoAccion::Ordenar(Modo::Mover) | TipoAccion::ParaBorrar
                ) {
                    v.push(ayuda(
                        "Queda un registro para deshacerlo: analizador deshacer <registro>",
                    ));
                }
                v.push(Line::from(vec![
                    Span::raw("Enter/s").fg(ACENTO).bold(),
                    Span::raw(": hacerlo  ·  ").fg(APAGADO),
                    Span::raw("n/Esc").fg(ACENTO).bold(),
                    Span::raw(": cancelar").fg(APAGADO),
                ]));
            } else {
                v.push(ayuda("Esc: volver"));
            }
            (" Vista previa ", v)
        }
        Accion::Ejecutando { plan, progreso, .. } => {
            let hechos = progreso.hechos.load(Ordering::Relaxed);
            let bytes = progreso.bytes.load(Ordering::Relaxed);
            let actual = progreso
                .actual
                .lock()
                .map(|p| relativa(&app.escaneo.raiz, &p))
                .unwrap_or_default();
            (
                " Trabajando ",
                vec![
                    Line::from(vec![
                        Span::raw(porcentaje(bytes, plan.bytes.max(1)))
                            .fg(ACENTO)
                            .bold(),
                        Span::raw(format!(
                            "  ·  {} de {} archivos  ·  {} de {}",
                            miles(hechos),
                            miles(plan.operaciones.len() as u64),
                            tamano(bytes),
                            tamano(plan.bytes)
                        )),
                    ]),
                    Line::raw(barra(bytes, plan.bytes.max(1), 50)).fg(ACENTO),
                    Line::raw(""),
                    ayuda(&actual),
                    Line::raw(""),
                    ayuda("Esc: cancelar (termina el archivo actual y no deja nada a medias)"),
                ],
            )
        }
        Accion::Listo { plan, resultado } => {
            let mut v = vec![
                Line::raw(format!(
                    "Listo: {} archivos ({}) en {}.",
                    miles(resultado.hechos as u64),
                    tamano(resultado.bytes),
                    duracion(resultado.duracion)
                ))
                .fg(Color::Green)
                .bold(),
            ];
            if resultado.cancelado {
                v.push(Line::raw("Se canceló: lo que faltaba quedó donde estaba.").fg(REVISAR));
            }
            if !resultado.fallidos.is_empty() {
                v.push(
                    Line::raw(format!(
                        "No se pudieron hacer {} (quedaron donde estaban):",
                        miles(resultado.fallidos.len() as u64)
                    ))
                    .fg(Color::Red),
                );
                for (op, err) in resultado.fallidos.iter().take(3) {
                    v.push(Line::raw(format!(
                        "  {}: {err}",
                        relativa(&app.escaneo.raiz, &op.origen)
                    )));
                }
            }
            if plan.tipo != TipoAccion::Deshacer {
                v.push(Line::raw(""));
                v.push(ayuda(&format!("Registro: {}", plan.registro.display())));
            }
            if matches!(
                plan.tipo,
                TipoAccion::Ordenar(Modo::Mover) | TipoAccion::ParaBorrar
            ) {
                v.push(ayuda(&format!(
                    "Para deshacer: analizador deshacer \"{}\"",
                    plan.registro.display()
                )));
            }
            v.push(Line::raw(""));
            v.push(ayuda(if resultado.hechos > 0 {
                "Cualquier tecla: volver a escanear para ver cómo quedó"
            } else {
                "Cualquier tecla: cerrar"
            }));
            (" Listo ", v)
        }
        Accion::Error(msg) => (
            " Error ",
            vec![
                Line::raw(msg.clone()).fg(Color::Red),
                Line::raw(""),
                ayuda("Cualquier tecla: cerrar"),
            ],
        ),
    };
    // La última línea son las teclas: va en una franja fija abajo, para que nunca se corte.
    let mut lineas = lineas;
    let teclas = lineas.pop().unwrap_or_default();
    let ancho = 100.min(f.area().width);
    let interior = (ancho.saturating_sub(2) as usize).max(1);
    // Cuántos renglones ocupa cada línea al ajustarse al ancho (+1 de margen por el corte
    // entre palabras, que puede usar un renglón más que la cuenta exacta).
    let renglones: usize = lineas
        .iter()
        .map(|l| l.width().max(1).div_ceil(interior) + usize::from(l.width() > interior))
        .sum();
    let alto = (renglones as u16 + 3).min(f.area().height);
    let area = centrado(f.area(), ancho, alto);
    f.render_widget(Clear, area);
    let b = bloque(titulo, true);
    let dentro = b.inner(area);
    f.render_widget(b, area);
    let [cuerpo, abajo] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(dentro);
    f.render_widget(Paragraph::new(lineas).wrap(Wrap { trim: false }), cuerpo);
    f.render_widget(Paragraph::new(teclas), abajo);
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
