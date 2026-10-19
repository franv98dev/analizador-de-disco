# Compila optimizado para la CPU de ESTA máquina (target-cpu=native).
# Ojo: el .exe resultante puede no arrancar en una CPU más vieja ("instrucción ilegal").
# Para llevar en un pendrive a otra PC usá el de los Releases, que anda en cualquier x64.
# Medido en la fase 6: escanear no se nota más rápido (depende del disco, no de la CPU),
# y BLAKE3 ya elige solo las instrucciones de la CPU al arrancar.
$env:RUSTFLAGS = "-C target-cpu=native -C target-feature=+crt-static"
cargo build --release
if ($?) { Write-Output "Listo: target\release\analizador.exe (solo para esta máquina)" }
