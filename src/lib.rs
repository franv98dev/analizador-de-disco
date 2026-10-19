//! Analizador de disco: separa lo que creó una persona de lo que es sistema o programas.
//! El escaneo es de solo lectura. Solo escriben `demo` (su propia carpeta de prueba) y
//! `exportar` (el informe, con vista previa y sin pisar archivos sin confirmación).

pub mod app;
pub mod arbol;
pub mod categoria;
pub mod demo;
pub mod escaner;
pub mod exclusiones;
pub mod exportar;
pub mod formato;
pub mod iconos;
pub mod informe;
pub mod resumen;
pub mod tui;
pub mod ui;
