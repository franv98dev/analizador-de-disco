//! Analizador de disco: separa lo que creó una persona de lo que es sistema o programas.
//! Todo es de solo lectura, salvo `demo`, que crea su propia carpeta de prueba.

pub mod categoria;
pub mod demo;
pub mod escaner;
pub mod exclusiones;
pub mod formato;
pub mod informe;
pub mod resumen;
