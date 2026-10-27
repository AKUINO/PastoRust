# Installation développeur — PastoRust et Pico 2 W

Ce guide prépare un poste Linux ou Windows avec VS Code pour compiler le projet,
tester son interface en simulation et flasher le firmware sur un Raspberry Pi
Pico 2 W (RP2350).

## 1. Matériel requis

- Raspberry Pi Pico 2 W.
- Câble USB capable de transférer des données (certains câbles ne servent qu'à
  l'alimentation).
- Pour flasher par USB : aucun adaptateur supplémentaire n'est requis.
- Pour le débogage pas à pas avec F5 : une sonde SWD compatible avec RP2350,
  reliée aux broches SWD du Pico. Le câble USB seul ne fournit pas le débogage
  SWD.

Les entrées utilisées par le firmware sont GP15 (numérique, pull-up interne) et
GP26 (ADC0). Ne jamais appliquer plus de 3,3 V aux broches du Pico.

## 2. Logiciels à installer

### Tous systèmes

1. Installer [Visual Studio Code](https://code.visualstudio.com/).
2. Installer [Git](https://git-scm.com/downloads).
3. Installer Rust avec [rustup](https://rustup.rs/). Le projet fixe Rust
   `1.91.0` dans `rust-toolchain.toml` et demande automatiquement la cible
   `thumbv8m.main-none-eabihf` lors de la première commande Cargo.
4. Installer `picotool` depuis les
   [versions officielles Raspberry Pi](https://github.com/raspberrypi/pico-sdk-tools/releases)
   et ajouter son exécutable au `PATH`. Vérifier avec `picotool version`.
5. Ouvrir le dossier du projet dans VS Code. Installer les extensions
   recommandées lorsqu'il le propose :
   - `rust-lang.rust-analyzer` : analyse et assistance Rust ;
   - `probe-rs.probe-rs` : débogage embarqué avec une sonde SWD ;
   - `vadimcn.vscode-lldb` : débogage de l'application locale.

Pour vérifier la sonde depuis un terminal, la commande `probe-rs` doit aussi
être disponible (`probe-rs --version`). Si elle ne l'est pas, installer les
outils depuis les
[instructions officielles probe-rs](https://probe.rs/docs/getting-started/installation/).

Le débogage SWD est facultatif : compilation, flash USB, console USB et Wi-Fi
fonctionnent sans sonde externe.

### Linux (Debian / Ubuntu)

Installer les outils de base et un terminal série :

```sh
sudo apt update
sudo apt install -y build-essential pkg-config minicom
```

Installer Rust si ce n'est pas déjà fait :

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Suivre les instructions affichées, puis ouvrir un nouveau terminal. Si Linux
refuse l'accès USB au Pico en mode BOOTSEL, vérifier les règles d'accès USB et
les permissions de l'utilisateur avant de relancer VS Code.

Pour autoriser `probe-rs` à accéder à une sonde sans lancer VS Code en
administrateur, installer les règles udev officielles :

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://probe.rs/files/69-probe-rs.rules |
  sudo tee /etc/udev/rules.d/69-probe-rs.rules >/dev/null
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Débrancher puis rebrancher la sonde après l'installation des règles. Si
l'utilisateur vient d'être ajouté au groupe `plugdev`, fermer puis rouvrir sa
session Linux pour appliquer ce changement.

### Windows

Installer Git for Windows et VS Code depuis leurs sites officiels, puis Rust
depuis [rustup.rs](https://rustup.rs/) (télécharger et exécuter
`rustup-init.exe`). Redémarrer le terminal après installation afin que `cargo`,
`rustc` et `picotool` soient trouvés dans le `PATH`.

Pour la cible MSVC, installer également les outils de compilation C++ de Visual
Studio (charge de travail **Développement Desktop en C++** et Windows SDK) si
le poste n'en dispose pas déjà ; ils fournissent l'éditeur de liens utilisé
pour l'application hôte.

Utiliser PowerShell ou le terminal intégré à VS Code. Si `picotool` ne détecte
pas la carte en mode BOOTSEL, vérifier que le pilote USB de la carte est
compatible avec `picotool` ; ne remplacer le pilote qu'en suivant les
instructions officielles Raspberry Pi.

## 3. Récupérer et préparer le projet

Après avoir poussé le dépôt sur GitHub, cloner le dépôt (remplacer les
placeholders par l'adresse fournie par le propriétaire) :

```sh
git clone https://github.com/<UTILISATEUR>/<DEPOT>.git
cd <DEPOT>
code .
```

Les binaires du firmware CYW43 sont nécessaires à la compilation du firmware ;
ils sont présents dans `firmware/` et leur licence est incluse dans le dossier.
Le code, la documentation et les ressources originaux de PastoRust sont sous
licence CC BY-SA 4.0 ; les binaires CYW43 et les dépendances conservent leurs
licences tierces. Conserver les notices et fichiers de licence correspondants
lorsqu'on partage le dépôt.

## 4. Compiler et tester

À la première commande Cargo, rustup installe la chaîne et la cible embarquée
définies par le projet. Depuis le terminal intégré VS Code :

```sh
cargo build --target thumbv8m.main-none-eabihf --features firmware --bin pastorust
```

Le même build est disponible via `Terminal > Run Build Task` ou `Ctrl+Shift+B`
(tâche **Build Pico 2 W**).

Pour tester l'interface sans carte, il faut aussi installer la cible hôte
appropriée. Sous Linux x86-64 :

```sh
rustup target add x86_64-unknown-linux-gnu
cargo run --target x86_64-unknown-linux-gnu --bin pastorust-web
```

Sous Windows x86-64 avec la chaîne MSVC :

```powershell
rustup target add x86_64-pc-windows-msvc
cargo run --target x86_64-pc-windows-msvc --bin pastorust-web
```

Puis ouvrir <http://127.0.0.1:8000>. Le serveur de simulation n'écoute que sur
la machine locale. Les noms de cible hôte sont explicités, car la configuration
Cargo du projet choisit par défaut la cible embarquée.

## 5. Flasher le Pico 2 W par USB

1. Connecter le Pico avec un câble USB de données.
2. Maintenir le bouton **BOOTSEL** en branchant la carte, puis relâcher le
   bouton : elle apparaît en mode bootloader USB.
3. Depuis la racine du projet, compiler et flasher le firmware :

```sh
cargo run --release --target thumbv8m.main-none-eabihf --features firmware --bin pastorust
```

La configuration `.cargo/config.toml` utilise `picotool` comme runner Cargo.
Après le build, Cargo demande à `picotool` de transférer et démarrer le programme.
Le même résultat est disponible dans VS Code avec la tâche **Flash Pico 2 W
(USB)**. Si le transfert échoue, vérifier le mode BOOTSEL, le câble, le `PATH`
de `picotool` et les permissions USB.

### Méthode de secours avec UF2

Si le flash direct ne fonctionne pas, construire le firmware puis créer un
fichier UF2 :

```sh
cargo build --release --target thumbv8m.main-none-eabihf --features firmware --bin pastorust
picotool uf2 convert target/thumbv8m.main-none-eabihf/release/pastorust -t elf target/thumbv8m.main-none-eabihf/release/pastorust.uf2
```

Laisser le Pico en mode BOOTSEL et copier
`target/thumbv8m.main-none-eabihf/release/pastorust.uf2` sur le lecteur USB qui
apparaît. La carte redémarre avec le nouveau firmware.

## 6. Vérifier la carte

Après le flash, le firmware démarre une console USB CDC, lit GP15 et GP26, puis
crée un point d'accès Wi-Fi et sert l'interface Web. Connecter un ordinateur ou
un téléphone à ce réseau, puis ouvrir `http://10.42.0.1`. Les détails réseau et
les valeurs configurées sont définis dans `main.rs`.

### Console USB

- **Linux** : repérer le port avec `ls /dev/ttyACM*`, puis par exemple :
  `minicom -D /dev/ttyACM0 -b 115200`.
- **Windows** : repérer le numéro `COM` dans le Gestionnaire de périphériques,
  puis l'ouvrir avec un terminal série tel que PuTTY. Le débit configuré n'a pas
  d'incidence sur USB CDC.

La console signale le démarrage et affiche périodiquement les mesures.

### Binaires de diagnostic

Les tâches VS Code **Flash USB Hello test**, **Flash Wi-Fi LED test** et
**Flash GP0 LED test** construisent et flashent des firmwares de diagnostic.
Leurs équivalents en ligne de commande sont :

```sh
cargo run --release --target thumbv8m.main-none-eabihf --features firmware --bin usb-hello
cargo run --release --target thumbv8m.main-none-eabihf --features firmware --bin wifi-led-test
cargo run --release --target thumbv8m.main-none-eabihf --features firmware --bin gpio0-led-test
```

Chaque binaire remplace l'application précédemment flashée ; reflasher
`pastorust` pour revenir à l'application principale.

## 7. Débogage pas à pas (facultatif)

Connecter une sonde SWD compatible au Pico et installer ses pilotes si le
système le demande. Dans VS Code, choisir **Déboguer Pico 2 W**, puis appuyer sur
F5. La configuration utilise `probe-rs`, cible la puce `RP2350` et flashe le
firmware de débogage.

### Vérifier la sonde

Brancher la sonde au PC, puis exécuter :

```sh
probe-rs list
```

La sortie doit contenir `Debug Probe (CMSIS-DAP)`. Si la sonde est marquée
`inaccessible` sous Linux, appliquer les règles udev décrites plus haut, puis
la rebrancher et recommencer.

Si `probe-rs info` indique que le firmware de la sonde est trop ancien
(minimum requis : 2.2.0), le mettre à jour avec le fichier
[`debugprobe.uf2` de la version officielle 2.3.1](https://github.com/raspberrypi/debugprobe/releases/tag/debugprobe-v2.3.1) :

1. Télécharger `debugprobe.uf2` dans les fichiers de cette version.
2. Débrancher la Debug Probe du PC, maintenir son bouton **BOOTSEL** en la
   rebranchant, puis relâcher le bouton. Elle apparaît comme lecteur USB
   `RPI-RP2`.
3. Copier `debugprobe.uf2` sur ce lecteur. La Probe redémarre avec le nouveau
   firmware ; la débrancher puis la rebrancher normalement.

Utiliser `debugprobe.uf2` pour la Debug Probe (accessoire), et non
`debugprobe_on_pico.uf2`, qui est destiné à un Pico utilisé comme sonde.
Cette mise à jour concerne le firmware de la sonde, pas celui du Pico cible.

Pour vérifier également la liaison SWD avec le Pico 2 W, relier les broches
SWD de la sonde à celles du Pico, avec une masse commune, puis exécuter :

```sh
probe-rs info --verbose
```

La section SWD doit afficher le cœur Cortex-M33 et la ROM RP235x. Une erreur
indiquant que la sonde ne prend pas en charge JTAG est normale : la Debug Probe
communique avec le Pico en SWD. Cette commande vérifie la liaison sans flasher
de firmware. Avec `probe-rs 0.32`, la commande `info` sans `--verbose` peut ne
pas autodétecter le nom RP2350, même si les composants SWD sont lisibles.
Pour les autres commandes, sélectionner explicitement `--protocol swd` et
`--chip RP2350`. Si plusieurs sondes sont branchées, sélectionner la bonne
avec `--probe VID:PID` (par exemple `--probe 2e8a:000c`).

Si `probe-rs list` détecte la sonde mais que la section SWD de
`probe-rs info --verbose` n'affiche aucun composant, la connexion USB de la
sonde fonctionne, mais pas encore la liaison SWD avec la cible. Vérifier le
branchement SWDIO vers SWDIO, SWCLK vers SWCLK et GND vers GND, ainsi que
l'alimentation du Pico (par exemple par son propre câble USB).
`probe-rs list` seul ne vérifie pas cette liaison.

Une fois la liaison SWD confirmée, choisir **Déboguer Pico 2 W** dans le
sélecteur de débogage de VS Code, puis appuyer sur **F5**. La configuration
compile en profil debug, flashe `pastorust` via SWD et démarre le débogueur.
Poser un point d'arrêt dans `main.rs` avant de lancer F5 : le programme démarre
et s'arrête à ce point. Sans point d'arrêt atteint, le firmware s'exécute
normalement. Cette opération remplace le firmware présent sur le Pico.

Pour déboguer le test de la LED GP0, choisir **Déboguer test LED GP0** dans le
sélecteur de débogage, poser un point d'arrêt dans `gpio0_led_test.rs`, puis
appuyer sur **F5**. Cette configuration compile le binaire en profil debug et
le flashe par SWD. Le programme démarre et s'arrête au point d'arrêt. Utiliser
**Continuer** (F5) pour reprendre l'exécution, **Pas à pas principal** (F10)
pour avancer sans entrer dans les fonctions appelées, ou **Pas à pas détaillé**
(F11) pour y entrer. Sans point d'arrêt atteint, le test fait clignoter la LED.

En ligne de commande, l'équivalent du build debug est :

```sh
cargo build --target thumbv8m.main-none-eabihf --features firmware --bin gpio0-led-test
```

Le profil `--release` utilisé pour un simple test d'exécution optimise le
programme et rend le débogage source moins fiable. Les deux configurations F5
ci-dessus utilisent le profil debug. Le test GP0 fait clignoter une LED externe
raccordée à GP0 avec une résistance adaptée. Il remplace le firmware présent
sur le Pico ; reflasher `pastorust` pour revenir à l'application principale.

Le bouton BOOTSEL et `picotool` suffisent pour le flash USB ordinaire ; ils ne
remplacent pas une sonde SWD pour l'exécution pas à pas.

## 8. Publier ou pousser les sources sur GitHub

### Avant tout push

Les paramètres Wi-Fi de développement sont actuellement écrits en clair dans
`main.rs` et également documentés dans le README. Ne pas publier ces valeurs
dans un dépôt public. Avant le premier push, remplacer les identifiants par des
valeurs de démonstration ou mettre en place une configuration locale non suivie
par Git ; si les vrais identifiants ont déjà été partagés, les renouveler.
Même un dépôt privé donne accès à son contenu aux collaborateurs autorisés.

Vérifier aussi les fichiers qui seront envoyés. Le `.gitignore` du projet exclut
`target/` (résultats de compilation), mais conserve `Cargo.lock`, les fichiers
VS Code, les sources et les fichiers firmware nécessaires.

### Premier push (dépôt GitHub vide)

Créer sur GitHub un dépôt vide, sans initialiser de README, licence ou
`.gitignore` distant. Puis, depuis le dossier PastoRust :

```sh
git init -b main
git add .
git status --short
```

Relire la liste : elle ne doit pas contenir `target/`, fichiers de configuration
personnels, ou secrets. Si tout est correct :

```sh
git commit -m "Initial commit: PastoRust"
git remote add origin https://github.com/<UTILISATEUR>/<DEPOT>.git
git push -u origin main
```

GitHub demandera une authentification. Utiliser le navigateur, Git Credential
Manager ou une clé SSH ; ne pas inscrire de jeton d'accès dans une commande, un
fichier du projet ou un message de commit.

### Pousser des changements ultérieurs

```sh
git status
git add <FICHIERS_A_INCLURE>
git diff --cached
git commit -m "Décrire le changement"
git push
```

Examiner `git diff --cached` avant chaque commit pour éviter d'envoyer des
identifiants ou des fichiers locaux. Si le dépôt GitHub existe déjà et a une
histoire (README ou commits), cloner ce dépôt puis y copier les sources plutôt
que de forcer un push depuis un historique indépendant.
