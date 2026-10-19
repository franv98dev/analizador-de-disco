//! Juegos de íconos. Los simples se ven en el cmd de Windows 10 (fuente Consolas);
//! emoji y Nerd Font quedan para Windows Terminal o terminales de Linux.

use crate::categoria::Categoria;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Iconos {
    #[default]
    Simples,
    Emoji,
    /// Requiere una fuente Nerd Font instalada en la terminal.
    Nerd,
}

impl Iconos {
    pub fn carpeta(self) -> &'static str {
        match self {
            Iconos::Simples => "►",
            Iconos::Emoji => "📁",
            Iconos::Nerd => "\u{f07b}",
        }
    }

    pub fn subir(self) -> &'static str {
        match self {
            Iconos::Simples => "◄",
            Iconos::Emoji => "🔙",
            Iconos::Nerd => "\u{f062}",
        }
    }

    pub fn proyecto(self) -> &'static str {
        match self {
            Iconos::Simples => "◊",
            Iconos::Emoji => "💻",
            Iconos::Nerd => "\u{f121}",
        }
    }

    pub fn revisar(self) -> &'static str {
        match self {
            Iconos::Simples => "♦",
            Iconos::Emoji => "👀",
            Iconos::Nerd => "\u{f06e}",
        }
    }

    pub fn excluido(self) -> &'static str {
        match self {
            Iconos::Simples => "×",
            Iconos::Emoji => "🚫",
            Iconos::Nerd => "\u{f05e}",
        }
    }

    pub fn categoria(self, c: Categoria) -> &'static str {
        match self {
            Iconos::Simples => "■",
            Iconos::Emoji => match c {
                Categoria::Fotos => "📷",
                Categoria::Videos => "🎬",
                Categoria::Musica => "🎵",
                Categoria::Grabaciones => "🎤",
                Categoria::Documentos => "📄",
                Categoria::Proyectos => "💻",
                Categoria::Comprimidos => "📦",
                Categoria::Instaladores => "💿",
                Categoria::Otros => "❓",
            },
            Iconos::Nerd => match c {
                Categoria::Fotos => "\u{f03e}",
                Categoria::Videos => "\u{f03d}",
                Categoria::Musica => "\u{f001}",
                Categoria::Grabaciones => "\u{f130}",
                Categoria::Documentos => "\u{f15c}",
                Categoria::Proyectos => "\u{f121}",
                Categoria::Comprimidos => "\u{f187}",
                Categoria::Instaladores => "\u{f019}",
                Categoria::Otros => "\u{f15b}",
            },
        }
    }
}
