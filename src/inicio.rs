//! Pantalla de inicio: qué revisar cuando el programa se abre con doble click (sin argumentos).
//! Como `app.rs`, solo tiene el estado y qué hace cada tecla; se testea sin terminal.

use std::path::PathBuf;

use ratatui::crossterm::event::KeyCode;
use ratatui::widgets::TableState;

use crate::unidades::Unidad;

#[derive(Debug, Clone, PartialEq)]
pub enum Opcion {
    Personal(PathBuf),
    Unidad(Unidad),
    Demo,
    Otra,
}

/// Lo que eligió la persona.
#[derive(Debug, Clone, PartialEq)]
pub enum Eleccion {
    Abrir(PathBuf),
    Demo,
    /// Volver a buscar discos (por si se enchufó un pendrive).
    Actualizar,
    Salir,
}

pub struct Inicio {
    pub opciones: Vec<Opcion>,
    pub tabla: TableState,
    /// Escribiendo la ruta de "Otra carpeta…".
    pub escribiendo: Option<String>,
    /// Un error para mostrar (por ejemplo, una carpeta que no existe).
    pub mensaje: Option<String>,
}

impl Inicio {
    pub fn new(unidades: Vec<Unidad>, personal: Option<PathBuf>) -> Self {
        let mut opciones: Vec<Opcion> = personal.into_iter().map(Opcion::Personal).collect();
        opciones.extend(unidades.into_iter().map(Opcion::Unidad));
        opciones.push(Opcion::Demo);
        opciones.push(Opcion::Otra);
        Inicio {
            opciones,
            tabla: TableState::default().with_selected(Some(0)),
            escribiendo: None,
            mensaje: None,
        }
    }

    pub fn seleccion(&self) -> Option<&Opcion> {
        self.tabla.selected().and_then(|i| self.opciones.get(i))
    }

    pub fn tecla(&mut self, code: KeyCode) -> Option<Eleccion> {
        self.mensaje = None;
        if let Some(texto) = &mut self.escribiendo {
            match code {
                KeyCode::Esc => self.escribiendo = None,
                KeyCode::Backspace => {
                    texto.pop();
                }
                KeyCode::Char(c) => texto.push(c),
                KeyCode::Enter => {
                    // "Copiar como ruta" del Explorador la pone entre comillas.
                    let ruta = texto.trim().trim_matches('"').trim().to_string();
                    if !ruta.is_empty() {
                        self.escribiendo = None;
                        return Some(Eleccion::Abrir(PathBuf::from(ruta)));
                    }
                }
                _ => {}
            }
            return None;
        }
        let n = self.opciones.len();
        let i = self.tabla.selected().unwrap_or(0);
        match code {
            KeyCode::Up => self.tabla.select(Some(i.saturating_sub(1))),
            KeyCode::Down => self.tabla.select(Some((i + 1).min(n - 1))),
            KeyCode::Home => self.tabla.select(Some(0)),
            KeyCode::End => self.tabla.select(Some(n - 1)),
            KeyCode::Char('q') | KeyCode::Esc => return Some(Eleccion::Salir),
            KeyCode::Char('r') => return Some(Eleccion::Actualizar),
            KeyCode::Enter | KeyCode::Right => {
                return match self.seleccion()? {
                    Opcion::Personal(p) => Some(Eleccion::Abrir(p.clone())),
                    Opcion::Unidad(u) => Some(Eleccion::Abrir(u.ruta.clone())),
                    Opcion::Demo => Some(Eleccion::Demo),
                    Opcion::Otra => {
                        self.escribiendo = Some(String::new());
                        None
                    }
                };
            }
            _ => {}
        }
        None
    }
}

/// La carpeta personal del usuario que corre el programa.
pub fn carpeta_personal() -> Option<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unidades::TipoUnidad;

    fn inicio() -> Inicio {
        let c = Unidad {
            ruta: PathBuf::from("C:\\"),
            tipo: TipoUnidad::Disco,
            total: Some(500),
            libre: Some(100),
        };
        Inicio::new(vec![c], Some(PathBuf::from("C:\\Users\\papa")))
    }

    #[test]
    fn el_orden_es_personal_discos_demo_y_otra() {
        let i = inicio();
        assert!(matches!(i.opciones[0], Opcion::Personal(_)));
        assert!(matches!(i.opciones[1], Opcion::Unidad(_)));
        assert_eq!(i.opciones[2], Opcion::Demo);
        assert_eq!(i.opciones[3], Opcion::Otra);
        assert_eq!(i.tabla.selected(), Some(0));
    }

    #[test]
    fn enter_abre_lo_elegido() {
        let mut i = inicio();
        assert_eq!(
            i.tecla(KeyCode::Enter),
            Some(Eleccion::Abrir(PathBuf::from("C:\\Users\\papa")))
        );
        i.tecla(KeyCode::Down);
        assert_eq!(
            i.tecla(KeyCode::Enter),
            Some(Eleccion::Abrir(PathBuf::from("C:\\")))
        );
        i.tecla(KeyCode::Down);
        assert_eq!(i.tecla(KeyCode::Enter), Some(Eleccion::Demo));
    }

    #[test]
    fn otra_carpeta_se_escribe_y_acepta_comillas() {
        let mut i = inicio();
        i.tecla(KeyCode::End);
        assert_eq!(i.tecla(KeyCode::Enter), None);
        assert!(i.escribiendo.is_some());
        // Mientras se escribe, la q es una letra, no "salir".
        for c in "\"D:\\Fotos de quinchos\"".chars() {
            assert_eq!(i.tecla(KeyCode::Char(c)), None);
        }
        assert_eq!(
            i.tecla(KeyCode::Enter),
            Some(Eleccion::Abrir(PathBuf::from("D:\\Fotos de quinchos")))
        );
        assert!(i.escribiendo.is_none());
    }

    #[test]
    fn esc_deja_de_escribir_sin_salir() {
        let mut i = inicio();
        i.tecla(KeyCode::End);
        i.tecla(KeyCode::Enter);
        assert_eq!(i.tecla(KeyCode::Esc), None);
        assert!(i.escribiendo.is_none());
        assert_eq!(i.tecla(KeyCode::Esc), Some(Eleccion::Salir));
    }

    #[test]
    fn enter_con_la_ruta_vacia_no_hace_nada() {
        let mut i = inicio();
        i.tecla(KeyCode::End);
        i.tecla(KeyCode::Enter);
        assert_eq!(i.tecla(KeyCode::Enter), None);
        assert!(i.escribiendo.is_some());
    }

    #[test]
    fn q_sale_y_r_actualiza() {
        let mut i = inicio();
        assert_eq!(i.tecla(KeyCode::Char('r')), Some(Eleccion::Actualizar));
        assert_eq!(i.tecla(KeyCode::Char('q')), Some(Eleccion::Salir));
    }

    #[test]
    fn no_se_pasa_de_los_bordes() {
        let mut i = inicio();
        i.tecla(KeyCode::Up);
        assert_eq!(i.tabla.selected(), Some(0));
        for _ in 0..10 {
            i.tecla(KeyCode::Down);
        }
        assert_eq!(i.tabla.selected(), Some(3));
    }
}
