# PastoRust — Raspberry Pi Pico 2 W

Le projet cible le RP2350A du Pico 2 W avec Rust stable et Embassy. Le firmware
lit GPIO15 (entrée avec pull-up) et ADC26 (ADC0) chaque seconde et sert
l'interface Web en Wi-Fi.

## VS Code

Installez les extensions recommandées par VS Code :

- rust-analyzer pour l'analyse et l'autocomplétion Rust ;
- probe-rs pour le débogage SWD.

Pour les instructions détaillées d'installation sous Linux et Windows, de
compilation, de flash USB et de publication du dépôt sur GitHub, voir le
[guide d'installation développeur](docs/INSTALLATION_DEVELOPPEUR.md).

Le premier lancement de Cargo installe automatiquement la version Rust et la
cible embarquée définies dans `rust-toolchain.toml`. `Ctrl+Shift+B` compile le
firmware. La tâche **Flash Pico 2 W (USB)** le flashe via `picotool` ; mettez la
carte en mode BOOTSEL avant de lancer la tâche. Si l'accès USB direct est refusé,
compilez puis convertissez l'ELF en UF2 avec
`picotool uf2 convert target/thumbv8m.main-none-eabihf/release/pastorust -t elf target/thumbv8m.main-none-eabihf/release/pastorust.uf2`
et copiez le fichier obtenu sur le lecteur BOOTSEL.
Le linker place la table de vecteurs RP2350 à `0x10000000`, le bloc de démarrage
à `0x10000200` et le code à `0x10000300`. Ces adresses respectent l'alignement
de 512 octets requis par la table de vecteurs du Cortex-M33.
Pour utiliser F5, il faut aussi une sonde de débogage SWD compatible ; le Pico
ne se débogue pas via son seul câble USB.

Le firmware expose une console série USB CDC (sans sonde ni câblage additionnel).
Après flash et redémarrage, ouvrez le port `/dev/ttyACM0` avec
`minicom -D /dev/ttyACM0 -b 115200` (le débit est ignoré par USB CDC). La console
affiche les étapes de démarrage Wi-Fi et les lectures GPIO15/ADC26 toutes les
10 secondes. Vérifiez le nom du port avec `ls /dev/ttyACM*`.

Pour tester l'USB indépendamment du Wi-Fi, mettez la carte en mode BOOTSEL et
lancez la tâche **Flash USB Hello test**, ou exécutez
`cargo run --release --features firmware --bin usb-hello`. Ce binaire ne lance
que l'USB CDC et écrit `Hello world` toutes les cinq secondes. Il remplace le
firmware principal jusqu'à ce que celui-ci soit reflashed.

Pour vérifier séparément le démarrage du contrôleur Wi-Fi et l'exécution du
firmware sans dépendre de la console USB, utilisez la tâche **Flash Wi-Fi LED
test** ou `cargo run --release --features firmware --bin wifi-led-test`. Après
l'initialisation du CYW43, la LED intégrée du Pico 2 W alterne allumée/éteinte
toutes les 500 ms. Ce test ne démarre pas le point d'accès ; reflashez ensuite
`pastorust` pour revenir à l'application.

Pour vérifier une LED externe raccordée à la broche physique GP0, utilisez la
tâche **Flash GP0 LED test** ou `cargo run --release --features firmware --bin
gpio0-led-test`. Ce test pilote directement le GPIO0 du RP2350 à l'état haut et
bas une seconde chacun, sans initialiser le Wi-Fi ni l'USB.

Pour le développement sans carte, choisissez **Run local Web application** dans
`Terminal > Run Task`, ou **Lancer l'application Web locale** dans le sélecteur
de lancement. Ouvrez ensuite http://127.0.0.1:8000. La page applicative est
commune ; la cible de compilation détermine le mode : hôte `simulation`, Pico
`device`. Le serveur hôte indique le mode `simulation`, ce qui révèle les
contrôles GPIO15/ADC26. Ces contrôles et leur API n'existent que dans le binaire
hôte.

Les pages à éditer sont dans `web/templates/` et les feuilles de style et
scripts dans `web/static/`. `build.rs` incorpore ces ressources à la compilation
dans le binaire hôte et le firmware, avec recompilation automatique quand un
fichier change. Relancez l'application locale après chaque modification. Le
firmware embarque les ressources et sert les mêmes routes que l'application PC,
avec `mode: "device"` et les lectures réelles de GPIO15/ADC26 ; les contrôles de
simulation restent ainsi masqués sur le Pico.

Installez `picotool` depuis les [versions officielles Raspberry Pi](https://github.com/raspberrypi/pico-sdk-tools/releases).
En ligne de commande, `cargo build --features firmware --bin pastorust` compile et
`cargo run --release --features firmware --bin pastorust` flashe le Pico ;
`cargo run --target x86_64-unknown-linux-gnu --bin pastorust-web` lance
l'application locale en mode simulation.

## Matériel et réseau

GP26 est utilisé ici comme exemple d'entrée ADC (ADC0) et GP15 comme entrée
numérique avec pull-up interne. GP15 est une entrée physique en mode Pico : pour
la faire passer à l'état bas, reliez-la à GND avec un interrupteur ou un bouton ;
à l'état ouvert, le pull-up la maintient à l'état haut. Le commutateur de la page
ne pilote que la simulation sur PC. Adaptez ces broches au câblage réel et ne
présentez jamais plus de 3,3 V aux entrées du Pico.

Au démarrage, le Pico 2 W crée le point d'accès WPA2 `PDO60-26001` (mot de passe
`past0.NET`), à l'adresse `10.42.0.1`. Connectez-y un téléphone ou un ordinateur,
puis ouvrez `http://10.42.0.1`. DHCP attribue aux clients une adresse entre
`10.42.0.10` et `10.42.0.100`. L'interface actualise les valeurs GPIO15/ADC26
chaque seconde ; les commandes de simulation n'apparaissent que dans l'application
PC.

Le serveur Web utilise HTTP sans chiffrement et le point d'accès ne fournit pas
d'accès Internet. N'utilisez pas ce réseau pour transporter d'autres données
sensibles. Les binaires du firmware CYW43 dans `firmware/` proviennent du dépôt
Embassy et sont fournis sous la licence incluse dans ce dossier.

## Licence

Sauf indication contraire, le code, la documentation et les ressources Web
originaux de PastoRust sont sous licence
[CC BY-SA 4.0 International](LICENSE). Cette licence ne s'applique pas aux
binaires CYW43 tiers de `firmware/` ni aux dépendances, qui conservent leurs
licences respectives. Voir [firmware/README.md](firmware/README.md).
