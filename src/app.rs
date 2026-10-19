//! Estado de la interfaz y qué hace cada tecla. No dibuja nada ni toca la terminal,
//! así se puede testear la navegación entera sin pantalla.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use crate::antiguedad;
use crate::arbol::{Arbol, Suma, Tipo};
use crate::borrable::{self, Item};
use crate::categoria::Categoria;
use crate::duplicados::{self, Duplicados, Progreso};
use crate::escaner::{Archivo, Escaneo, Excluido};
use crate::exportar::{self, Plan};
use crate::iconos::Iconos;
use crate::resumen::{Grupo, Resumen, Totales};

/// Cuántos archivos listar como máximo en una lista de "más pesados".
pub const MAX_ARCHIVOS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Disco,
    Clasificado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pestana {
    Categorias,
    Pesados,
    Antiguedad,
    Borrable,
    Duplicados,
    Proyectos,
    Revisar,
    Excluido,
}

impl Pestana {
    pub const TODAS: [Pestana; 8] = [
        Pestana::Categorias,
        Pestana::Pesados,
        Pestana::Antiguedad,
        Pestana::Borrable,
        Pestana::Duplicados,
        Pestana::Proyectos,
        Pestana::Revisar,
        Pestana::Excluido,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Pestana::Categorias => "Categorías",
            Pestana::Pesados => "Pesados",
            Pestana::Antiguedad => "Antigüedad",
            Pestana::Borrable => "Borrable",
            Pestana::Duplicados => "Duplicados",
            Pestana::Proyectos => "Proyectos",
            Pestana::Revisar => "Revisar",
            Pestana::Excluido => "Excluido",
        }
    }
}

/// Una fila del panel del disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entrada {
    Subir,
    Carpeta(usize),
    Archivo(u32),
}

/// De dónde salió una lista de archivos (para poder volver).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filtro {
    Categoria(Categoria),
    Anio(Option<i32>),
}

/// Lo que muestra el panel derecho.
#[derive(Debug, Clone)]
pub enum Derecha {
    Categorias,
    /// Archivos debajo del foco, de mayor a menor. Sin filtro es la pestaña Pesados.
    Archivos(Option<Filtro>, Vec<u32>),
    Anios(Vec<(Option<i32>, Suma)>),
    Borrable(Vec<Item>),
    /// Índices de los grupos de duplicados con algún archivo debajo del foco.
    Duplicados(Vec<usize>),
    /// Las copias de un grupo.
    Copias(usize),
    Grupos(Vec<Grupo>),
    Excluidos(Vec<Excluido>),
}

impl Derecha {
    /// Se llegó con Enter desde otra vista: ← vuelve en vez de cambiar de pestaña.
    fn es_subvista(&self) -> bool {
        matches!(self, Derecha::Archivos(Some(_), _) | Derecha::Copias(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Exportar {
    /// Elegir la carpeta de destino.
    Destino(String),
    /// Vista previa: qué se va a escribir. `sobrescribe` = hay archivos que ya existen.
    Confirmar {
        plan: Plan,
        sobrescribe: bool,
    },
    Listo(String),
    Error(String),
}

/// Búsqueda de duplicados corriendo en otro hilo.
pub struct Busqueda {
    pub progreso: Arc<Progreso>,
    pub candidatos: usize,
    pub bytes: u64,
    rx: Receiver<io::Result<Duplicados>>,
}

pub struct App<'a> {
    pub escaneo: &'a Escaneo,
    pub arbol: Arbol,
    pub resumen: Resumen,
    /// Sugerencias de "probablemente borrable" de todo el escaneo.
    pub borrable: Vec<Item>,
    pub duplicados: Option<Duplicados>,
    pub busqueda: Option<Busqueda>,
    /// Mensaje para mostrar (por ejemplo, si falló la búsqueda de duplicados).
    pub aviso: Option<String>,
    pub iconos: Iconos,
    pub panel: Panel,
    /// Carpeta abierta en el panel izquierdo.
    pub actual: usize,
    pub entradas: Vec<Entrada>,
    pub tabla_izq: TableState,
    /// Carpeta que se clasifica a la derecha: la seleccionada a la izquierda o, si es un
    /// archivo, la abierta.
    pub foco: usize,
    pub pestana: Pestana,
    pub derecha: Derecha,
    pub tabla_der: TableState,
    /// Fechas de la categoría seleccionada debajo del foco.
    pub detalle: Option<Totales>,
    pub exportar: Option<Exportar>,
    pub salir: bool,
}

impl<'a> App<'a> {
    pub fn new(escaneo: &'a Escaneo, iconos: Iconos) -> Self {
        let mut app = App {
            escaneo,
            arbol: Arbol::de(escaneo),
            resumen: Resumen::de(escaneo),
            borrable: borrable::analizar(escaneo, None),
            duplicados: None,
            busqueda: None,
            aviso: None,
            iconos,
            panel: Panel::Disco,
            actual: Arbol::RAIZ,
            entradas: Vec::new(),
            tabla_izq: TableState::default(),
            foco: Arbol::RAIZ,
            pestana: Pestana::Categorias,
            derecha: Derecha::Categorias,
            tabla_der: TableState::default(),
            detalle: None,
            exportar: None,
            salir: false,
        };
        app.abrir(Arbol::RAIZ, None);
        app
    }

    pub fn archivo(&self, i: u32) -> &Archivo {
        &self.escaneo.archivos[i as usize]
    }

    pub fn seleccion_izq(&self) -> Option<Entrada> {
        self.tabla_izq
            .selected()
            .and_then(|i| self.entradas.get(i).copied())
    }

    /// Cantidad de filas del panel derecho.
    pub fn filas_der(&self) -> usize {
        match &self.derecha {
            Derecha::Categorias => Categoria::TODAS.len(),
            Derecha::Archivos(_, v) => v.len(),
            Derecha::Anios(v) => v.len(),
            Derecha::Borrable(v) => v.len(),
            Derecha::Duplicados(v) => v.len(),
            Derecha::Copias(g) => self.grupo(*g).map_or(0, |g| g.archivos.len()),
            Derecha::Grupos(v) => v.len(),
            Derecha::Excluidos(v) => v.len(),
        }
    }

    pub fn grupo(&self, g: usize) -> Option<&duplicados::GrupoDuplicado> {
        self.duplicados.as_ref().and_then(|d| d.grupos.get(g))
    }

    pub fn tecla(&mut self, k: KeyEvent) {
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.salir = true;
            return;
        }
        if self.exportar.is_some() {
            self.tecla_exportar(k.code);
            return;
        }
        match k.code {
            KeyCode::Char('q') => self.salir = true,
            KeyCode::Tab | KeyCode::BackTab => {
                self.panel = match self.panel {
                    Panel::Disco => Panel::Clasificado,
                    Panel::Clasificado => Panel::Disco,
                };
            }
            KeyCode::Char('e') => {
                let destino = std::env::current_dir()
                    .map(|d| d.display().to_string())
                    .unwrap_or_else(|_| ".".to_string());
                self.exportar = Some(Exportar::Destino(destino));
            }
            KeyCode::Char('d') => self.alternar_busqueda(),
            KeyCode::Char(c @ '1'..='8') => {
                self.cambiar_pestana(Pestana::TODAS[c as usize - '1' as usize]);
            }
            _ => match self.panel {
                Panel::Disco => self.tecla_disco(k.code),
                Panel::Clasificado => self.tecla_clasificado(k.code),
            },
        }
    }

    fn tecla_disco(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End => {
                mover(&mut self.tabla_izq, self.entradas.len(), code);
                self.actualizar_foco();
            }
            KeyCode::Enter | KeyCode::Right => match self.seleccion_izq() {
                Some(Entrada::Subir) => self.subir(),
                Some(Entrada::Carpeta(id))
                    if !matches!(self.arbol.nodo(id).tipo, Tipo::Excluido(_)) =>
                {
                    self.abrir(id, None)
                }
                _ => {}
            },
            KeyCode::Backspace | KeyCode::Left => self.subir(),
            _ => {}
        }
    }

    fn tecla_clasificado(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End => {
                let filas = self.filas_der();
                mover(&mut self.tabla_der, filas, code);
                self.actualizar_detalle();
            }
            KeyCode::Left => {
                if self.derecha.es_subvista() {
                    self.volver();
                } else {
                    self.cambiar_pestana(anterior(self.pestana));
                }
            }
            KeyCode::Right => {
                if !self.derecha.es_subvista() {
                    self.cambiar_pestana(siguiente(self.pestana));
                }
            }
            KeyCode::Backspace | KeyCode::Esc => self.volver(),
            KeyCode::Enter => self.entrar_derecha(),
            _ => {}
        }
    }

    fn entrar_derecha(&mut self) {
        let Some(i) = self.tabla_der.selected() else {
            return;
        };
        let destino: Option<(PathBuf, Option<u32>)> = match &self.derecha {
            Derecha::Categorias => {
                let filtro = Filtro::Categoria(Categoria::TODAS[i]);
                self.abrir_lista(filtro);
                return;
            }
            Derecha::Anios(v) => {
                if let Some(&(anio, _)) = v.get(i) {
                    self.abrir_lista(Filtro::Anio(anio));
                }
                return;
            }
            Derecha::Duplicados(v) => {
                if let Some(&g) = v.get(i) {
                    self.derecha = Derecha::Copias(g);
                    self.tabla_der.select(Some(0));
                }
                return;
            }
            Derecha::Archivos(_, v) => v.get(i).map(|&a| self.donde_esta(a)),
            Derecha::Copias(g) => self
                .grupo(*g)
                .and_then(|g| g.archivos.get(i))
                .map(|&a| self.donde_esta(a)),
            Derecha::Borrable(v) => v.get(i).map(|item| match item.archivo {
                Some(a) => self.donde_esta(a),
                None => (item.ruta.clone(), None),
            }),
            Derecha::Grupos(v) => v.get(i).map(|g| (g.ruta.to_path_buf(), None)),
            Derecha::Excluidos(v) => v
                .get(i)
                .map(|x| (x.ruta.parent().unwrap_or(&x.ruta).to_path_buf(), None)),
        };
        if let Some((carpeta, archivo)) = destino {
            self.ir_a(&carpeta, archivo);
        }
    }

    /// Carpeta de un archivo, para saltar a él en el panel del disco.
    fn donde_esta(&self, a: u32) -> (PathBuf, Option<u32>) {
        let ruta = &self.archivo(a).ruta;
        (ruta.parent().unwrap_or(ruta).to_path_buf(), Some(a))
    }

    fn abrir_lista(&mut self, filtro: Filtro) {
        let archivos = self.mas_pesados(self.foco, Some(filtro));
        self.derecha = Derecha::Archivos(Some(filtro), archivos);
        self.tabla_der.select(Some(0));
    }

    /// Desde una subvista, vuelve a la vista de la que se vino con esa fila seleccionada.
    fn volver(&mut self) {
        let seleccion = match &self.derecha {
            Derecha::Archivos(Some(Filtro::Categoria(c)), _) => Some(*c as usize),
            Derecha::Archivos(Some(Filtro::Anio(a)), _) => {
                let a = *a;
                self.cargar_pestana();
                if let Derecha::Anios(v) = &self.derecha {
                    let i = v.iter().position(|(x, _)| *x == a);
                    self.tabla_der.select(i);
                }
                self.actualizar_detalle();
                return;
            }
            Derecha::Copias(g) => {
                let g = *g;
                self.cargar_pestana();
                if let Derecha::Duplicados(v) = &self.derecha {
                    let i = v.iter().position(|x| *x == g);
                    self.tabla_der.select(i);
                }
                return;
            }
            _ => return,
        };
        self.cargar_pestana();
        self.tabla_der.select(seleccion);
        self.actualizar_detalle();
    }

    /// Abre `carpeta` a la izquierda y, si se indica, selecciona ese archivo.
    pub fn ir_a(&mut self, carpeta: &Path, archivo: Option<u32>) {
        if let Some(id) = self.arbol.buscar(carpeta) {
            let seleccion = archivo.map(Entrada::Archivo);
            self.abrir(id, seleccion);
            self.panel = Panel::Disco;
        }
    }

    /// Abre una carpeta en el panel izquierdo y selecciona `seleccion` (o la primera fila).
    pub fn abrir(&mut self, id: usize, seleccion: Option<Entrada>) {
        self.actual = id;
        self.entradas = self.entradas_de(id);
        let i = seleccion
            .and_then(|s| self.entradas.iter().position(|e| *e == s))
            .unwrap_or(
                if self.entradas.len() > 1 && self.entradas[0] == Entrada::Subir {
                    1
                } else {
                    0
                },
            );
        self.tabla_izq =
            TableState::default().with_selected((!self.entradas.is_empty()).then_some(i));
        self.actualizar_foco();
    }

    pub fn subir(&mut self) {
        if let Some(padre) = self.arbol.nodo(self.actual).padre {
            let vengo_de = self.actual;
            self.abrir(padre, Some(Entrada::Carpeta(vengo_de)));
        }
    }

    /// Subcarpetas y archivos mezclados de mayor a menor; los excluidos al final.
    fn entradas_de(&self, id: usize) -> Vec<Entrada> {
        let nodo = self.arbol.nodo(id);
        let mut v = Vec::with_capacity(nodo.hijos.len() + nodo.archivos.len() + 1);
        if nodo.padre.is_some() {
            v.push(Entrada::Subir);
        }
        let (mut h, mut a) = (
            nodo.hijos.iter().peekable(),
            nodo.archivos.iter().peekable(),
        );
        loop {
            let tam_h = h.peek().map(|&&x| self.arbol.nodo(x).total().bytes);
            let tam_a = a.peek().map(|&&x| self.archivo(x).tamano);
            match (tam_h, tam_a) {
                (Some(th), Some(ta)) if th >= ta => v.push(Entrada::Carpeta(*h.next().unwrap())),
                (_, Some(_)) => v.push(Entrada::Archivo(*a.next().unwrap())),
                (Some(_), None) => v.push(Entrada::Carpeta(*h.next().unwrap())),
                (None, None) => break,
            }
        }
        // Los excluidos pesan 0 (no se recorren): mejor al final que mezclados con lo vacío.
        v.sort_by_key(|e| matches!(e, Entrada::Carpeta(id) if matches!(self.arbol.nodo(*id).tipo, Tipo::Excluido(_))));
        v
    }

    fn actualizar_foco(&mut self) {
        let foco = match self.seleccion_izq() {
            Some(Entrada::Carpeta(id)) => id,
            _ => self.actual,
        };
        let cambio = foco != self.foco;
        self.foco = foco;
        if cambio || !self.derecha.es_subvista() {
            self.cargar_pestana();
        }
    }

    fn cambiar_pestana(&mut self, p: Pestana) {
        self.pestana = p;
        self.cargar_pestana();
    }

    /// Recalcula el panel derecho para el foco actual.
    fn cargar_pestana(&mut self) {
        let base = self.arbol.ruta(self.foco);
        let bajo = |r: &Path| r.starts_with(&base);
        self.derecha = match self.pestana {
            Pestana::Categorias => Derecha::Categorias,
            Pestana::Pesados => Derecha::Archivos(None, self.mas_pesados(self.foco, None)),
            Pestana::Antiguedad => Derecha::Anios(antiguedad::por_anio(
                self.arbol
                    .archivos_bajo(self.foco)
                    .iter()
                    .map(|&i| self.archivo(i))
                    .filter(|a| a.revisar.is_none()),
            )),
            Pestana::Borrable => Derecha::Borrable(
                self.borrable
                    .iter()
                    .filter(|i| bajo(&i.ruta))
                    .cloned()
                    .collect(),
            ),
            Pestana::Duplicados => Derecha::Duplicados(match &self.duplicados {
                Some(d) => (0..d.grupos.len())
                    .filter(|&g| {
                        d.grupos[g]
                            .archivos
                            .iter()
                            .any(|&a| bajo(&self.archivo(a).ruta))
                    })
                    .collect(),
                None => Vec::new(),
            }),
            Pestana::Proyectos => Derecha::Grupos(
                self.resumen
                    .proyectos
                    .iter()
                    .filter(|g| bajo(&g.ruta))
                    .cloned()
                    .collect(),
            ),
            Pestana::Revisar => Derecha::Grupos(
                self.resumen
                    .revisar
                    .iter()
                    .filter(|g| bajo(&g.ruta))
                    .cloned()
                    .collect(),
            ),
            Pestana::Excluido => Derecha::Excluidos(
                self.escaneo
                    .excluidos
                    .iter()
                    .filter(|x| bajo(&x.ruta))
                    .cloned()
                    .collect(),
            ),
        };
        let filas = self.filas_der();
        let sel = self
            .tabla_der
            .selected()
            .unwrap_or(0)
            .min(filas.saturating_sub(1));
        self.tabla_der = TableState::default().with_selected((filas > 0).then_some(sel));
        self.actualizar_detalle();
    }

    fn actualizar_detalle(&mut self) {
        self.detalle = match (&self.derecha, self.tabla_der.selected()) {
            (Derecha::Categorias, Some(i)) => {
                let c = Categoria::TODAS[i];
                let mut t = Totales::default();
                for &a in self.arbol.archivos_bajo(self.foco) {
                    let a = self.archivo(a);
                    if a.categoria == c && a.revisar.is_none() {
                        t.sumar(a);
                    }
                }
                Some(t)
            }
            _ => None,
        };
    }

    /// Los más pesados debajo de una carpeta. Con filtro, solo lo del usuario
    /// (sin "revisar") de esa categoría o ese año; sin filtro, todo.
    pub fn mas_pesados(&self, id: usize, filtro: Option<Filtro>) -> Vec<u32> {
        let pasa = |a: &Archivo| match filtro {
            None => true,
            Some(Filtro::Categoria(c)) => a.categoria == c && a.revisar.is_none(),
            Some(Filtro::Anio(anio)) => {
                antiguedad::anio(a.modificado) == anio && a.revisar.is_none()
            }
        };
        let mut v: Vec<u32> = self
            .arbol
            .archivos_bajo(id)
            .iter()
            .copied()
            .filter(|&a| pasa(self.archivo(a)))
            .collect();
        let tam = |i: &u32| std::cmp::Reverse(self.archivo(*i).tamano);
        if v.len() > MAX_ARCHIVOS {
            v.select_nth_unstable_by_key(MAX_ARCHIVOS, tam);
            v.truncate(MAX_ARCHIVOS);
        }
        v.sort_by_key(tam);
        v
    }

    /// `d`: busca duplicados en otro hilo, o cancela la búsqueda en curso.
    fn alternar_busqueda(&mut self) {
        if let Some(b) = &self.busqueda {
            b.progreso.cancelar.store(true, Ordering::Relaxed);
            return;
        }
        let candidatos = duplicados::candidatos(self.escaneo);
        let progreso = Arc::new(Progreso::default());
        let (tx, rx) = mpsc::channel();
        self.busqueda = Some(Busqueda {
            progreso: Arc::clone(&progreso),
            candidatos: candidatos.len(),
            bytes: duplicados::bytes_candidatos(&candidatos),
            rx,
        });
        thread::spawn(move || {
            let _ = tx.send(duplicados::buscar(candidatos, &progreso, None));
        });
        self.aviso = None;
        self.cambiar_pestana(Pestana::Duplicados);
    }

    /// Revisa si terminó la búsqueda de duplicados. Se llama en cada vuelta del bucle.
    pub fn tick(&mut self) {
        let Some(b) = &self.busqueda else {
            return;
        };
        let resultado = match b.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(io::Error::other("la búsqueda se interrumpió")),
        };
        self.busqueda = None;
        match resultado {
            Ok(d) if d.cancelado => self.aviso = Some("Búsqueda cancelada.".to_string()),
            Ok(d) => {
                self.borrable = borrable::analizar(self.escaneo, Some(&d));
                self.duplicados = Some(d);
            }
            Err(err) => self.aviso = Some(format!("No se pudieron buscar duplicados: {err}")),
        }
        self.cargar_pestana();
    }

    fn tecla_exportar(&mut self, code: KeyCode) {
        let Some(estado) = self.exportar.take() else {
            return;
        };
        self.exportar = match (estado, code) {
            (_, KeyCode::Esc) => None,
            (Exportar::Destino(mut d), KeyCode::Char(c)) => {
                d.push(c);
                Some(Exportar::Destino(d))
            }
            (Exportar::Destino(mut d), KeyCode::Backspace) => {
                d.pop();
                Some(Exportar::Destino(d))
            }
            (Exportar::Destino(d), KeyCode::Enter) => {
                let plan =
                    exportar::planificar(Path::new(d.trim()), self.escaneo, chrono::Local::now());
                let sobrescribe = !plan.existentes().is_empty();
                Some(Exportar::Confirmar { plan, sobrescribe })
            }
            (Exportar::Confirmar { plan, sobrescribe }, KeyCode::Enter | KeyCode::Char('s')) => {
                match exportar::escribir(&plan, self.escaneo, self.duplicados.as_ref(), sobrescribe)
                {
                    Ok(()) => Some(Exportar::Listo(format!(
                        "Listo:\n{}\n{}",
                        plan.markdown.display(),
                        plan.csv.display()
                    ))),
                    Err(err) => Some(Exportar::Error(err.to_string())),
                }
            }
            (Exportar::Confirmar { .. }, KeyCode::Char('n')) => None,
            (Exportar::Listo(_) | Exportar::Error(_), _) => None,
            (otro, _) => Some(otro),
        };
    }
}

fn mover(t: &mut TableState, filas: usize, code: KeyCode) {
    if filas == 0 {
        return;
    }
    let i = t.selected().unwrap_or(0);
    let pagina = 15;
    let nuevo = match code {
        KeyCode::Up => i.saturating_sub(1),
        KeyCode::Down => (i + 1).min(filas - 1),
        KeyCode::PageUp => i.saturating_sub(pagina),
        KeyCode::PageDown => (i + pagina).min(filas - 1),
        KeyCode::Home => 0,
        KeyCode::End => filas - 1,
        _ => i,
    };
    t.select(Some(nuevo));
}

fn siguiente(p: Pestana) -> Pestana {
    let i = Pestana::TODAS.iter().position(|x| *x == p).unwrap_or(0);
    Pestana::TODAS[(i + 1) % Pestana::TODAS.len()]
}

fn anterior(p: Pestana) -> Pestana {
    let i = Pestana::TODAS.iter().position(|x| *x == p).unwrap_or(0);
    Pestana::TODAS[(i + Pestana::TODAS.len() - 1) % Pestana::TODAS.len()]
}
