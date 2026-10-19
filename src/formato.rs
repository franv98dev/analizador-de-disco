//! Números y tamaños en formato argentino: `1.234.567` y `1,5 GB`.

use std::time::{Duration, SystemTime};

pub fn tamano(bytes: u64) -> String {
    const UNIDADES: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut valor = bytes as f64 / 1024.0;
    let mut unidad = 0;
    while valor >= 1024.0 && unidad < UNIDADES.len() - 1 {
        valor /= 1024.0;
        unidad += 1;
    }
    format!("{valor:.1} {}", UNIDADES[unidad]).replace('.', ",")
}

pub fn miles(n: u64) -> String {
    let digitos = n.to_string();
    let mut s = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            s.push('.');
        }
        s.push(c);
    }
    s
}

pub fn duracion(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 60.0 {
        format!("{s:.2} s").replace('.', ",")
    } else {
        let total = d.as_secs();
        format!("{} min {:02} s", total / 60, total % 60)
    }
}

pub fn fecha(t: Option<SystemTime>) -> String {
    match t {
        Some(t) => chrono::DateTime::<chrono::Local>::from(t)
            .format("%Y-%m-%d")
            .to_string(),
        None => "—".to_string(),
    }
}

pub fn porcentaje(parte: u64, total: u64) -> String {
    if total == 0 {
        return "0,0%".to_string();
    }
    format!("{:.1}%", parte as f64 * 100.0 / total as f64).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanos_legibles() {
        assert_eq!(tamano(0), "0 B");
        assert_eq!(tamano(1023), "1023 B");
        assert_eq!(tamano(1024), "1,0 KB");
        assert_eq!(tamano(1536), "1,5 KB");
        assert_eq!(tamano(5 * 1024 * 1024 * 1024), "5,0 GB");
        assert_eq!(tamano(u64::MAX), "16384,0 PB");
    }

    #[test]
    fn separador_de_miles() {
        assert_eq!(miles(0), "0");
        assert_eq!(miles(999), "999");
        assert_eq!(miles(1000), "1.000");
        assert_eq!(miles(1234567), "1.234.567");
    }

    #[test]
    fn duraciones() {
        assert_eq!(duracion(Duration::from_millis(1500)), "1,50 s");
        assert_eq!(duracion(Duration::from_secs(125)), "2 min 05 s");
    }

    #[test]
    fn porcentajes() {
        assert_eq!(porcentaje(1, 4), "25,0%");
        assert_eq!(porcentaje(1, 0), "0,0%");
    }
}
