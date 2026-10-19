//! Analizador de disco: separa lo que creó una persona de lo que es sistema o programas.
//! El escaneo es de solo lectura. Solo escriben `demo` (su propia carpeta de prueba) y
//! `exportar` (el informe, con vista previa y sin pisar archivos sin confirmación).
//! `duplicados` es lo único que lee el contenido de los archivos, y solo para leer.

pub mod acciones;
pub mod antiguedad;
pub mod app;
pub mod arbol;
pub mod borrable;
pub mod categoria;
pub mod demo;
pub mod duplicados;
pub mod escaner;
pub mod exclusiones;
pub mod exportar;
pub mod formato;
pub mod fotos;
pub mod iconos;
pub mod informe;
pub mod resumen;
pub mod tui;
pub mod ui;
