#!/bin/sh
# Compila optimizado para la CPU de ESTA máquina (target-cpu=native).
# Ojo: el binario resultante puede no arrancar en una CPU más vieja ("instrucción ilegal").
# Para llevar a otra máquina usá el de los Releases, que anda en cualquier x64.
# Medido en la fase 6: escanear no se nota más rápido (depende del disco, no de la CPU),
# y BLAKE3 ya elige solo las instrucciones de la CPU al arrancar.
set -e
RUSTFLAGS="-C target-cpu=native" cargo build --release
echo "Listo: target/release/analizador (solo para esta máquina)"
