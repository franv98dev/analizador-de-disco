//! Discos, pendrives y unidades de red conectados, para la pantalla de inicio.
//! En Windows son las letras de unidad; en Linux, los puntos de montaje de `/proc/mounts`.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoUnidad {
    Disco,
    Extraible,
    Red,
}

impl TipoUnidad {
    pub fn nombre(self) -> &'static str {
        match self {
            TipoUnidad::Disco => "Disco",
            TipoUnidad::Extraible => "Pendrive/externo",
            TipoUnidad::Red => "Red",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unidad {
    pub ruta: PathBuf,
    pub tipo: TipoUnidad,
    pub total: Option<u64>,
    pub libre: Option<u64>,
}

/// Las unidades conectadas ahora, con su tamaño y espacio libre.
pub fn detectar() -> Vec<Unidad> {
    candidatas()
        .into_iter()
        .map(|(ruta, tipo)| Unidad {
            total: fs4::total_space(&ruta).ok(),
            libre: fs4::available_space(&ruta).ok(),
            ruta,
            tipo,
        })
        .collect()
}

#[cfg(windows)]
fn candidatas() -> Vec<(PathBuf, TipoUnidad)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    // Valores de GetDriveTypeW.
    const EXTRAIBLE: u32 = 2;
    const FIJA: u32 = 3;
    const RED: u32 = 4;

    // Desde C: — A: y B: son disqueteras y preguntarles puede trabar.
    (b'C'..=b'Z')
        .filter_map(|letra| {
            let raiz = format!("{}:\\", letra as char);
            let ancha: Vec<u16> = std::ffi::OsStr::new(&raiz)
                .encode_wide()
                .chain(Some(0))
                .collect();
            // SAFETY: `ancha` es una cadena UTF-16 terminada en 0 que vive durante la llamada.
            let tipo = match unsafe { GetDriveTypeW(ancha.as_ptr()) } {
                FIJA => TipoUnidad::Disco,
                EXTRAIBLE => TipoUnidad::Extraible,
                RED => TipoUnidad::Red,
                // Lectoras de CD/DVD, discos RAM o letras sin nada.
                _ => return None,
            };
            let ruta = PathBuf::from(raiz);
            ruta.is_dir().then_some((ruta, tipo))
        })
        .collect()
}

#[cfg(unix)]
fn candidatas() -> Vec<(PathBuf, TipoUnidad)> {
    std::fs::read_to_string("/proc/mounts")
        .map(|t| puntos_de_montaje(&t))
        .unwrap_or_else(|_| vec![(PathBuf::from("/"), TipoUnidad::Disco)])
}

/// Sistemas de archivos de discos de verdad (y de red). Quedan afuera `proc`, `tmpfs`, `overlay`…
const SISTEMAS_DE_DISCO: &[&str] = &[
    "ext2", "ext3", "ext4", "xfs", "btrfs", "f2fs", "zfs", "vfat", "exfat", "ntfs", "ntfs3",
    "fuseblk",
];
const SISTEMAS_DE_RED: &[&str] = &["nfs", "nfs4", "cifs", "smb3", "fuse.sshfs"];
/// Montajes que no son para revisar aunque estén en un disco.
const MONTAJES_DE_SISTEMA: &[&str] = &["/boot", "/snap", "/var", "/proc", "/sys", "/dev", "/run"];
/// Donde se montan los pendrives y discos externos.
const MONTAJES_EXTRAIBLES: &[&str] = &["/media", "/run/media", "/mnt"];

/// Puntos de montaje que vale la pena ofrecer, a partir del texto de `/proc/mounts`.
pub fn puntos_de_montaje(texto: &str) -> Vec<(PathBuf, TipoUnidad)> {
    let mut v: Vec<(PathBuf, TipoUnidad)> = Vec::new();
    for linea in texto.lines() {
        let campos: Vec<&str> = linea.split_whitespace().collect();
        let [_, montaje, sistema, ..] = campos.as_slice() else {
            continue;
        };
        let montaje = sin_escapes(montaje);
        let debajo = |base: &str| montaje == base || montaje.starts_with(&format!("{base}/"));
        let tipo = if SISTEMAS_DE_RED.contains(sistema) {
            TipoUnidad::Red
        } else if !SISTEMAS_DE_DISCO.contains(sistema) {
            continue;
        } else if MONTAJES_EXTRAIBLES.iter().any(|b| debajo(b)) {
            TipoUnidad::Extraible
        } else if MONTAJES_DE_SISTEMA.iter().any(|b| debajo(b)) {
            continue;
        } else {
            TipoUnidad::Disco
        };
        let ruta = PathBuf::from(montaje);
        if !v.iter().any(|(r, _)| *r == ruta) {
            v.push((ruta, tipo));
        }
    }
    v
}

/// `/proc/mounts` escribe los espacios y otros caracteres raros como `\040` (octal).
fn sin_escapes(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // Se miran los bytes (y no `&s[..]`): así una letra con acento después de `\` no corta
        // un carácter por la mitad.
        let octal = b
            .get(i + 1..i + 4)
            .filter(|d| d.iter().all(|c| (b'0'..=b'7').contains(c)));
        if b[i] == b'\\'
            && let Some(d) = octal
        {
            bytes.push(d.iter().fold(0u8, |n, c| n.wrapping_mul(8) + (c - b'0')));
            i += 4;
            continue;
        }
        bytes.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "\
sysfs /sys sysfs rw,nosuid 0 0
proc /proc proc rw,nosuid 0 0
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
/dev/nvme0n1p1 /boot/efi vfat rw 0 0
tmpfs /run tmpfs rw 0 0
/dev/sda1 /home ext4 rw 0 0
/dev/sdb1 /media/fran/MI\\040PENDRIVE vfat rw 0 0
/dev/sdc1 /run/media/fran/Backup ntfs3 rw 0 0
//nas/fotos /mnt/nas cifs rw 0 0
/dev/loop0 /snap/core/123 squashfs ro 0 0
overlay /var/lib/docker/overlay2/x overlay rw 0 0
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
";

    #[test]
    fn lee_los_montajes_que_sirven() {
        let v = puntos_de_montaje(MOUNTS);
        let rutas: Vec<(String, TipoUnidad)> = v
            .into_iter()
            .map(|(r, t)| (r.to_string_lossy().into_owned(), t))
            .collect();
        assert_eq!(
            rutas,
            vec![
                ("/".to_string(), TipoUnidad::Disco),
                ("/home".to_string(), TipoUnidad::Disco),
                ("/media/fran/MI PENDRIVE".to_string(), TipoUnidad::Extraible),
                ("/run/media/fran/Backup".to_string(), TipoUnidad::Extraible),
                ("/mnt/nas".to_string(), TipoUnidad::Red),
            ]
        );
    }

    #[test]
    fn espacios_escapados() {
        assert_eq!(sin_escapes(r"/media/a\040b"), "/media/a b");
        assert_eq!(sin_escapes(r"/sin/escapes"), "/sin/escapes");
        assert_eq!(sin_escapes(r"/termina\04"), r"/termina\04");
        assert_eq!(sin_escapes(r"/raro\ñandú"), r"/raro\ñandú");
    }

    #[test]
    fn detecta_al_menos_el_disco_del_sistema() {
        let v = detectar();
        let raiz = if cfg!(windows) { "C:\\" } else { "/" };
        assert!(
            v.iter().any(|u| u.ruta == std::path::Path::new(raiz)),
            "{v:?}"
        );
    }
}
