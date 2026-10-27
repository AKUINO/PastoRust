// La cible embarquée ne fournit pas la bibliothèque standard ; ce binaire se limite à `core`.
#![no_std]
// Le point d'entrée est fourni par le runtime Cortex-M plutôt que par le démarrage natif Rust.
#![no_main]

// Outils de formatage qui fonctionnent sans allocation dynamique.
use core::fmt;
// Permet d'écrire le résultat du formatage dans des chaînes à capacité fixe.
use core::fmt::Write as _;
// État partagé entre tâches sans verrou conventionnel ni allocation.
use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

// Pilote du module Wi-Fi CYW43 via le contrôleur PIO du RP2350.
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
// Active la sortie RTT de `defmt` sans importer de symbole dans ce module.
use defmt_rtt as _;
// Permet de lancer les tâches asynchrones Embassy depuis l'initialisation.
use embassy_executor::Spawner;
// Socket TCP servant à recevoir les requêtes HTTP.
use embassy_net::tcp::TcpSocket;
// Types pour configurer et partager la pile réseau IPv4 Embassy.
use embassy_net::{Config, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4};
// Pilote ADC et handler d'interruption nécessaires à la conversion analogique asynchrone.
use embassy_rp::adc::{Adc, Channel, Config as AdcConfig, InterruptHandler as AdcInterruptHandler};
// Abstractions GPIO d'entrée et de sortie pour les broches utilisées.
use embassy_rp::gpio::{Input, Level, Output, Pull};
// Type matériel requis par le handler d'interruption du PIO.
use embassy_rp::peripherals::PIO0;
// PIO Embassy et gestionnaire d'interruption du périphérique PIO0.
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
// Pilote USB et gestionnaire de son interruption matérielle.
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
// Relie les interruptions matérielles aux pilotes et expose le canal DMA nécessaire au Wi-Fi.
use embassy_rp::{bind_interrupts, dma};
// Mutex compatible avec le partage d'une file entre tâches et interruptions.
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
// File asynchrone qui sépare la production des logs de leur émission sur USB.
use embassy_sync::channel::Channel as SyncChannel;
// Durées et temporisations asynchrones qui ne bloquent pas le processeur.
use embassy_time::Duration;
// Classe USB CDC-ACM afin que la console apparaisse comme un port série.
use embassy_usb::class::cdc_acm::CdcAcmClass;
// État persistant requis par la classe USB CDC-ACM.
use embassy_usb::class::cdc_acm::State as CdcState;
// Constructeur du périphérique USB et type de sa configuration.
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig, UsbDevice};
// Serveur DHCP et mécanisme de gestion des baux distribués aux clients Wi-Fi.
use esp_hal_dhcp_server::simple_leaser::SimpleDhcpLeaser;
// Paramètres du serveur DHCP.
use esp_hal_dhcp_server::structs::DhcpServerConfig;
// Adresses IPv4 du serveur DHCP et fonction qui le fait tourner.
use esp_hal_dhcp_server::{Ipv4Addr, run_dhcp_server};
// Chaînes à taille fixe pour garantir une utilisation mémoire bornée sans tas.
use heapless::String;
// Fournit le gestionnaire de panique prévu pour le firmware de débogage.
use panic_probe as _;
// Stockage initialisé une fois pour les objets nécessitant une durée de vie statique.
use static_cell::StaticCell;

#[path = "src/web.rs"]
// Réutilise les ressources web compilées, partagées avec le mode de simulation.
mod web;

// SSID et mot de passe présentés par le point d'accès Wi-Fi de l'appareil.
const WIFI_SSID: &str = "PDO60-26001";
const WIFI_PASSWORD: &str = "past0.NET";
// Adresse fixe de l'appareil sur son réseau privé ; elle sert aussi de passerelle.
const ACCESS_POINT_IP: Ipv4Address = Ipv4Address::new(10, 42, 0, 1);
// Taille des tampons réutilisés par les sockets TCP pour maîtriser la RAM.
const TCP_BUFFER_SIZE: usize = 2048;
// Limite de taille acceptée pour les en-têtes HTTP reçus.
const HTTP_REQUEST_SIZE: usize = 1024;
// Capacité maximale d'une ligne de log construite sans allocation dynamique.
const USB_LOG_LINE_SIZE: usize = 64;

// Dernier niveau lu sur GPIO15, publié aux autres tâches et à l'API d'état.
static GPIO15_HIGH: AtomicBool = AtomicBool::new(false);
// Dernière conversion ADC de GPIO26, partagée avec l'API HTTP.
static ADC26_SAMPLE: AtomicU16 = AtomicU16::new(0);
// Nombre de lignes abandonnées faute de place dans la file de logs USB.
static USB_LOG_DROPS: AtomicU16 = AtomicU16::new(0);
// File bornée de logs ; un producteur plein n'est pas bloqué, sa perte est comptée.
static USB_LOGS: SyncChannel<CriticalSectionRawMutex, String<USB_LOG_LINE_SIZE>, 16> =
    SyncChannel::new();

// Associe chaque interruption matérielle au pilote Embassy chargé de la traiter.
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
    ADC_IRQ_FIFO => AdcInterruptHandler;
    USBCTRL_IRQ => UsbInterruptHandler<embassy_rp::peripherals::USB>;
});

// Alias raccourcis pour les types USB ayant une durée de vie statique exigée par Embassy.
type UsbDeviceType = UsbDevice<'static, UsbDriver<'static, embassy_rp::peripherals::USB>>;
type UsbCdcClass = CdcAcmClass<'static, UsbDriver<'static, embassy_rp::peripherals::USB>>;

// Formate puis met un message dans la file sans attendre que l'hôte USB soit disponible.
fn usb_log(args: fmt::Arguments<'_>) {
    // Une capacité fixe permet d'éviter l'allocateur absent en environnement embarqué.
    let mut line = String::<USB_LOG_LINE_SIZE>::new();
    // Si le format dépasse la capacité ou que la file est pleine, le compteur garde trace de la perte.
    if line.write_fmt(args).is_err() || USB_LOGS.try_send(line).is_err() {
        USB_LOG_DROPS.fetch_add(1, Ordering::Relaxed);
    }
}

// Tâche permanente qui exécute la pile protocolaire du contrôleur USB.
#[embassy_executor::task]
async fn usb_device_task(mut device: UsbDeviceType) -> ! {
    // `run` ne termine normalement pas : le périphérique doit rester opérationnel en continu.
    device.run().await
}

// Attend la console série USB et transfère les lignes de log vers l'hôte.
#[embassy_executor::task]
async fn usb_log_task(mut class: UsbCdcClass) -> ! {
    loop {
        // Évite de vider la file avant qu'un terminal hôte ait ouvert la connexion série.
        class.wait_connection().await;
        // Confirme à l'utilisateur que la console est connectée.
        let _ = class
            .write_packet(b"\r\nPastoRust USB console connected\r\n")
            .await;

        loop {
            // Lit puis remet à zéro le compteur pour rapporter les pertes accumulées depuis le dernier contrôle.
            let dropped = USB_LOG_DROPS.swap(0, Ordering::Relaxed);
            if dropped > 0 {
                // La chaîne bornée évite d'allouer pour afficher l'avertissement.
                let mut line = String::<USB_LOG_LINE_SIZE>::new();
                let _ = write!(line, "WARNING: {dropped} USB log lines dropped");
                // Une erreur d'écriture indique que la liaison n'est plus utilisable ; on attendra sa reconnexion.
                if class.write_packet(line.as_bytes()).await.is_err()
                    || class.write_packet(b"\r\n").await.is_err()
                {
                    break;
                }
            }

            // Attend un nouveau message au lieu de scruter la file et consommer inutilement du processeur.
            let line = USB_LOGS.receive().await;
            // Ajoute un séparateur lisible entre les messages ; une erreur termine cette session USB.
            if class.write_packet(line.as_bytes()).await.is_err()
                || class.write_packet(b"\r\n").await.is_err()
            {
                break;
            }
        }
    }
}

// Fait progresser en continu le pilote Wi-Fi afin que la radio puisse traiter ses événements.
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

// Traite les événements de la pile réseau Embassy pour permettre les échanges IP.
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn sample_inputs_task(
    mut adc: Adc<'static, embassy_rp::adc::Async>,
    mut adc_input: Channel<'static>,
    gpio_input: Input<'static>,
) -> ! {
    // L'horodatage sert à produire un résumé périodique plutôt qu'un log à chaque mesure.
    let mut last_log = embassy_time::Instant::now();
    loop {
        // Lit le niveau numérique et publie immédiatement la valeur pour les autres tâches.
        let gpio15_high = gpio_input.is_high();
        GPIO15_HIGH.store(gpio15_high, Ordering::Relaxed);
        // La conversion ADC est asynchrone ; l'attente laisse le planificateur exécuter d'autres tâches.
        match adc.read(&mut adc_input).await {
            // Conserve la valeur la plus récente pour l'API HTTP et le prochain log périodique.
            Ok(sample) => ADC26_SAMPLE.store(sample, Ordering::Relaxed),
            Err(error) => {
                // L'erreur est visible à la fois dans RTT et dans la console USB.
                defmt::warn!("ADC GPIO26 read failed: {:?}", error);
                usb_log(format_args!("ERROR ADC26 read failed: {:?}", error));
            }
        }
        // Un rapport toutes les dix secondes suffit au suivi sans saturer la sortie série.
        if last_log.elapsed() >= Duration::from_secs(10) {
            usb_log(format_args!(
                "Inputs: GPIO15={} ADC26={}",
                if gpio15_high { "high" } else { "low" },
                ADC26_SAMPLE.load(Ordering::Relaxed)
            ));
            // Redémarre la période à partir de maintenant après l'émission du rapport.
            last_log = embassy_time::Instant::now();
        }
        // Cadence de mesure d'une seconde pour limiter le travail et stabiliser les échantillons.
        embassy_time::Timer::after_secs(1).await;
    }
}

// Configure puis maintient le serveur DHCP du réseau du point d'accès.
#[embassy_executor::task]
async fn dhcp_task(stack: Stack<'static>) {
    // L'appareil s'annonce comme passerelle du réseau privé.
    let gateway = Ipv4Addr::new(10, 42, 0, 1);
    let config = DhcpServerConfig {
        // Adresse locale utilisée par le serveur DHCP.
        ip: gateway,
        // Les baux expirent après une heure pour permettre leur réattribution.
        lease_time: Duration::from_secs(3600),
        // Les clients reçoivent l'adresse du point d'accès comme passerelle.
        gateways: &[gateway],
        // Masque /24 correspondant aux adresses 10.42.0.x.
        subnet: Some(Ipv4Addr::new(255, 255, 255, 0)),
        // Aucun DNS externe n'est annoncé sur ce réseau privé.
        dns: &[],
        // Les clients ne sont pas redirigés vers un portail captif.
        use_captive_portal: false,
    };
    // La plage attribuée aux clients évite l'adresse réservée à l'appareil en .1.
    let mut leaser = SimpleDhcpLeaser {
        // Commence à .10 pour garder les premières adresses disponibles à d'éventuels usages fixes.
        start: Ipv4Addr::new(10, 42, 0, 10),
        // Termine à .100 pour plafonner le nombre d'adresses susceptibles d'être louées.
        end: Ipv4Addr::new(10, 42, 0, 100),
        // Initialise sans bail actif puisque le serveur vient de démarrer.
        leases: Default::default(),
    };

    // Une erreur de démarrage du service est signalée au lieu d'être ignorée.
    if let Err(error) = run_dhcp_server(stack, config, &mut leaser).await {
        defmt::error!("DHCP server failed to bind: {:?}", error);
    }
}

// Accepte les connexions HTTP séquentiellement pour garder une empreinte mémoire déterministe.
#[embassy_executor::task]
async fn http_task(stack: Stack<'static>) {
    // Tampons TCP réutilisés pour éviter allocation dynamique et consommation par connexion.
    let mut rx_buffer = [0; TCP_BUFFER_SIZE];
    let mut tx_buffer = [0; TCP_BUFFER_SIZE];

    loop {
        // Un seul socket est vivant à la fois et emprunte les deux tampons.
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        // Libère le serveur si un client se connecte puis cesse de communiquer.
        socket.set_timeout(Some(Duration::from_secs(10)));

        // Le port 80 est le port HTTP conventionnel ; en cas d'échec, la boucle réessaie.
        if let Err(error) = socket.accept(80).await {
            defmt::warn!("HTTP accept failed: {:?}", error);
            continue;
        }

        // Traite une requête et journalise les erreurs avant de fermer la connexion.
        if let Err(error) = serve_http_request(&mut socket).await {
            defmt::warn!("HTTP request failed: {:?}", error);
        }
        // Termine explicitement la connexion après la réponse HTTP.
        socket.close();
    }
}

// Lit et route une requête HTTP minimale, en propageant les erreurs de transport TCP.
async fn serve_http_request(socket: &mut TcpSocket<'_>) -> Result<(), embassy_net::tcp::Error> {
    // Tampon de requête à capacité fixe pour borner les ressources consommées par un client.
    let mut request = [0; HTTP_REQUEST_SIZE];
    // Nombre d'octets valides effectivement reçus dans le tampon.
    let mut request_len = 0;
    // Indique si le séparateur de fin d'en-têtes HTTP a été trouvé.
    let mut headers_complete = false;

    // Lit jusqu'à la fin des en-têtes ou jusqu'à ce que le tampon soit plein.
    while request_len < request.len() {
        // Écrit à la suite des octets déjà lus afin de ne pas déplacer les données.
        let count = socket.read(&mut request[request_len..]).await?;
        // Une lecture vide signifie que le client a fermé la connexion.
        if count == 0 {
            break;
        }
        // Étend la portion du tampon considérée comme valide après une lecture partielle.
        request_len += count;
        // Les en-têtes HTTP se terminent par CRLFCRLF ; cette recherche accepte plusieurs lectures TCP.
        if request[..request_len]
            .windows(4)
            .any(|window| window == b"\r\n\r\n")
        {
            headers_complete = true;
            break;
        }
    }

    // La première ligne est nécessaire pour identifier méthode et chemin.
    let Some(line_end) = request[..request_len]
        .iter()
        .position(|byte| *byte == b'\n')
    else {
        // Une requête dépourvue de ligne initiale ne peut pas être routée.
        return send_http_response(socket, "400 Bad Request", "text/plain", b"Bad Request").await;
    };
    // Retire le CR optionnel précédant LF pour normaliser la ligne HTTP.
    let request_line = request[..line_end]
        .strip_suffix(b"\r")
        .unwrap_or(&request[..line_end]);
    // Découpe sans allocation la ligne de requête sur ses espaces séparateurs.
    let mut parts = request_line.split(|byte| *byte == b' ');
    // Le premier champ détermine l'action HTTP demandée.
    let Some(method) = parts.next() else {
        return send_http_response(socket, "400 Bad Request", "text/plain", b"Bad Request").await;
    };
    // Le second champ désigne la ressource à servir.
    let Some(path) = parts.next() else {
        return send_http_response(socket, "400 Bad Request", "text/plain", b"Bad Request").await;
    };

    // Refuse une requête dont les en-têtes dépassent le tampon fixe : aucune route n'est évaluée.
    if !headers_complete {
        return send_http_response(
            socket,
            "431 Request Header Fields Too Large",
            "text/plain",
            b"Request Too Large",
        )
        .await;
    }

    // Seule la lecture est implémentée ; rejeter explicitement les autres méthodes évite une réponse trompeuse.
    if method != b"GET" {
        return send_http_response(
            socket,
            "405 Method Not Allowed",
            "text/plain",
            b"Method Not Allowed",
        )
        .await;
    }

    // Ignore la query string pour que `/style.css?v=1` corresponde à la ressource `/style.css`.
    let path = path.split(|byte| *byte == b'?').next().unwrap_or(path);
    // Chaque route renvoie une ressource compilée ou un état généré à partir des entrées observées.
    match path {
        b"/" => {
            // Sert la page d'accueil embarquée sans accès disque ni lecture réseau supplémentaire.
            send_http_response(
                socket,
                "200 OK",
                "text/html; charset=utf-8",
                web::INDEX_HTML,
            )
            .await
        }
        b"/style.css" => {
            // Sert la feuille de style avec un type MIME adapté au navigateur.
            send_http_response(socket, "200 OK", "text/css; charset=utf-8", web::STYLE_CSS).await
        }
        b"/app.js" => {
            // Sert le code navigateur en déclarant son type JavaScript et son encodage.
            send_http_response(
                socket,
                "200 OK",
                "text/javascript; charset=utf-8",
                web::APP_JS,
            )
            .await
        }
        b"/api/state" => {
            // La réponse JSON est bornée et suffit aux valeurs exposées par cette API.
            let mut body = String::<128>::new();
            // Les atomiques rendent disponibles les mesures les plus récentes sans verrou partagé.
            if write!(
                body,
                "{{\"mode\":\"{}\",\"gpio15\":{},\"adc26\":{}}}",
                web::MODE,
                GPIO15_HIGH.load(Ordering::Relaxed),
                ADC26_SAMPLE.load(Ordering::Relaxed)
            )
            .is_err()
            {
                return send_http_response(
                    socket,
                    "500 Internal Server Error",
                    "text/plain",
                    b"Internal Server Error",
                )
                .await;
            }
            // L'API est consommée par le navigateur ; JSON décrit explicitement les champs de mesure.
            send_http_response(socket, "200 OK", "application/json", body.as_bytes()).await
        }
        // Une route inconnue reçoit un statut 404 plutôt qu'un contenu de réussite par défaut.
        _ => send_http_response(socket, "404 Not Found", "text/plain", b"Not Found").await,
    }
}

// Construit une réponse HTTP bornée puis envoie en-tête et corps sur le socket.
async fn send_http_response(
    socket: &mut TcpSocket<'_>,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), embassy_net::tcp::Error> {
    // Capacité fixe du petit en-tête commun à toutes les réponses du serveur.
    let mut header = String::<160>::new();
    // Le Content-Length explicite aide le client à délimiter le corps sans connexion persistante.
    if write!(
        header,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    )
    .is_err()
    {
        // Impossible d'émettre un en-tête valide si la chaîne fixe est trop petite.
        return Err(embassy_net::tcp::Error::ConnectionReset);
    }

    // Garantit l'envoi complet de l'en-tête même si TCP n'accepte qu'une partie des octets.
    write_all(socket, header.as_bytes()).await?;
    // Envoie le corps après les en-têtes, dans l'ordre requis par HTTP.
    write_all(socket, body).await?;
    // Vide les données restantes du socket pour qu'elles soient transmises avant sa fermeture.
    socket.flush().await
}

// Écrit l'intégralité d'une tranche, car une écriture TCP peut être partielle.
async fn write_all(
    socket: &mut TcpSocket<'_>,
    bytes: &[u8],
) -> Result<(), embassy_net::tcp::Error> {
    // Position du premier octet qui n'a pas encore été accepté par le socket.
    let mut written = 0;
    // Répète les écritures jusqu'à la transmission de toute la tranche.
    while written < bytes.len() {
        // N'envoie que la partie restante pour ne pas retransmettre les octets déjà écrits.
        let count = socket.write(&bytes[written..]).await?;
        // Zéro octet de progression rendrait la boucle infinie ; on traite cela comme une rupture.
        if count == 0 {
            return Err(embassy_net::tcp::Error::ConnectionReset);
        }
        // Avance au prochain octet restant après une écriture réussie.
        written += count;
    }
    // `Ok` n'est renvoyé qu'une fois tous les octets transmis.
    Ok(())
}

// Point d'entrée Embassy : initialise le matériel puis lance les services concurrents.
#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(spawner: Spawner) {
    // Configure les périphériques RP2350 et fournit leurs handles aux pilotes Embassy.
    let p = embassy_rp::init(Default::default());

    // Place un premier message dans la file ; il sera émis dès qu'un terminal USB se connectera.
    usb_log(format_args!("PastoRust starting on RP2350"));
    // Crée le pilote USB associé à la ressource matérielle et à ses interruptions.
    let usb_driver = UsbDriver::new(p.USB, Irqs);
    // Descripteurs et paramètres d'identification visibles par l'ordinateur hôte.
    let usb_config = {
        // Identifiants USB de fabricant/produit ; ils distinguent ce périphérique côté hôte.
        let mut config = UsbConfig::new(0xc0de, 0xcafe);
        // Chaînes visibles dans les informations descriptives présentées à l'ordinateur.
        config.manufacturer = Some("PastoRust");
        config.product = Some("PastoRust diagnostics");
        config.serial_number = Some("PDO60-26001");
        // Courant maximal déclaré au bus USB, en unités de 2 mA (100 signifie 200 mA).
        config.max_power = 100;
        // Taille standard des paquets du point de terminaison de contrôle USB 0.
        config.max_packet_size_0 = 64;
        config
    };
    // Les descripteurs doivent vivre aussi longtemps que le périphérique USB ; StaticCell fournit cette durée.
    let mut usb_builder = {
        // Réserve l'espace persistant pour les descripteurs de configuration annoncés au PC.
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        // Réserve les descripteurs BOS qui décrivent les capacités USB supplémentaires.
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        // Le protocole de contrôle USB a besoin d'un tampon persistant pour ses échanges.
        static CONTROL_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();
        UsbBuilder::new(
            usb_driver,
            usb_config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            // Aucun descripteur supplémentaire n'est fourni ici.
            &mut [],
            CONTROL_BUFFER.init([0; 64]),
        )
    };
    // La classe CDC-ACM fournit l'interface série utilisée pour les logs.
    let cdc_class = {
        // La classe garde son état entre appels ; il doit donc rester valide pour toute sa durée de vie.
        static CDC_STATE: StaticCell<CdcState> = StaticCell::new();
        // 64 octets correspondent à la taille maximale des paquets de données CDC utilisés ici.
        CdcAcmClass::new(&mut usb_builder, CDC_STATE.init(CdcState::new()), 64)
    };
    // Finalise la configuration USB avant de lancer les tâches qui la font fonctionner.
    let usb_device = usb_builder.build();
    // Sépare la gestion du périphérique de la transmission des logs pour qu'elles progressent indépendamment.
    spawner.spawn(usb_device_task(usb_device).expect("failed to create USB device task"));
    spawner.spawn(usb_log_task(cdc_class).expect("failed to create USB log task"));
    usb_log(format_args!("USB CDC console ready"));

    // Initialise le convertisseur analogique avec le handler d'interruption déclaré plus haut.
    let adc = Adc::new(p.ADC, Irqs, AdcConfig::default());
    // Configure GPIO26 comme entrée analogique sans résistance de tirage.
    let adc_input = Channel::new_pin(p.PIN_26, Pull::None);
    // Configure GPIO15 en entrée avec tirage haut, utile lorsque le signal externe est ouvert.
    let gpio_input = Input::new(p.PIN_15, Pull::Up);
    // Lance la tâche qui met à jour en continu les valeurs partagées des deux entrées.
    spawner.spawn(
        sample_inputs_task(adc, adc_input, gpio_input).expect("failed to create GPIO/ADC task"),
    );

    // Maintient la broche d'alimentation du module Wi-Fi à l'état actif.
    let pwr = Output::new(p.PIN_23, Level::Low);
    // Initialise la sélection du périphérique SPI à l'état inactif (niveau haut).
    let cs = Output::new(p.PIN_25, Level::High);
    // Prend possession du bloc PIO pour programmer l'interface SPI logicielle.
    let mut pio = Pio::new(p.PIO0, Irqs);
    // Configure la liaison PIO-SPI avec horloge, broches, interruptions et DMA pour le Wi-Fi.
    let spi = PioSpi::new(
        // Bloc PIO servant à exécuter le programme SPI sans solliciter le CPU bit par bit.
        &mut pio.common,
        // Machine d'état réservée à ce bus SPI.
        pio.sm0,
        // Diviseur par défaut définissant la fréquence d'horloge SPI du module.
        DEFAULT_CLOCK_DIVIDER,
        // Interruption PIO utilisée pour signaler les événements du pilote.
        pio.irq0,
        cs,
        // Broche d'horloge et broche de réception/transmission reliées au module CYW43.
        p.PIN_24,
        p.PIN_29,
        // DMA transfère les données SPI sans copie active par le processeur.
        dma::Channel::new(p.DMA_CH0, Irqs),
    );

    // Firmware, paramètres radio et données réglementaires requis par le composant CYW43.
    let firmware = cyw43::aligned_bytes!("firmware/43439A0.bin");
    let clm = cyw43::aligned_bytes!("firmware/43439A0_clm.bin");
    let nvram = cyw43::aligned_bytes!("firmware/nvram_rp2040.bin");
    // L'état du pilote doit rester alloué pour toute la durée de vie du module Wi-Fi.
    static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = CYW43_STATE.init(cyw43::State::new());
    // Informe la console avant l'étape de chargement, qui peut prendre un certain temps.
    usb_log(format_args!("Loading CYW43 Wi-Fi firmware"));
    // Démarre le pilote et récupère la carte réseau, le canal de contrôle et son runner asynchrone.
    let (net_device, mut control, cyw43_runner) =
        cyw43::new(state, pwr, spi, firmware, nvram).await;
    // Le runner traite les interruptions et événements en arrière-plan.
    spawner.spawn(cyw43_task(cyw43_runner).expect("failed to create Wi-Fi driver task"));

    // Configure le pays et les paramètres radio avant d'émettre le réseau.
    usb_log(format_args!("Initializing Wi-Fi country/PHY settings"));
    control.init(clm).await;
    // Journalise le nom du réseau avant son activation.
    usb_log(format_args!("Starting WPA2 AP: {WIFI_SSID}"));
    // Privilégie les performances pour cet appareil alimenté en continu.
    control
        .set_power_management(cyw43::PowerManagementMode::Performance)
        .await;
    // Crée un point d'accès WPA2 sur le canal 6 avec les identifiants configurés.
    control.start_ap_wpa2(WIFI_SSID, WIFI_PASSWORD, 6).await;
    usb_log(format_args!("Wi-Fi AP started at {ACCESS_POINT_IP}"));

    // Générateur matériel utilisé pour initialiser la séquence aléatoire de la pile réseau.
    let mut rng = embassy_rp::clocks::RoscRng;
    // Ressources de pile statiques dimensionnées pour le nombre de sockets prévus.
    static STACK_RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
    // Attache la carte Wi-Fi à une interface IPv4 fixe et crée le runner de la pile.
    let (stack, net_runner) = embassy_net::new(
        net_device,
        Config::ipv4_static(StaticConfigV4 {
            // Adresse fixe cohérente avec l'IP de passerelle et le serveur DHCP.
            address: Ipv4Cidr::new(ACCESS_POINT_IP, 24),
            // Les clients et l'appareil partagent ce réseau sans routeur distinct.
            gateway: Some(ACCESS_POINT_IP),
            // Ne diffuse aucun DNS, car ce réseau local ne fournit pas de résolution externe.
            dns_servers: Default::default(),
        }),
        STACK_RESOURCES.init(StackResources::new()),
        rng.next_u64(),
    );
    // Le runner fait progresser la pile IP indépendamment des tâches applicatives.
    spawner.spawn(net_task(net_runner).expect("failed to create network stack task"));
    // Lance les deux services accessibles aux clients : attribution d'adresse et interface HTTP.
    usb_log(format_args!("Starting DHCP and HTTP services"));
    spawner.spawn(dhcp_task(stack).expect("failed to create DHCP server task"));
    spawner.spawn(http_task(stack).expect("failed to create HTTP server task"));
    // Donne à l'utilisateur l'adresse de connexion une fois les services démarrés.
    usb_log(format_args!(
        "Ready: connect to {WIFI_SSID}, then browse http://{ACCESS_POINT_IP}"
    ));

    // Émet également le statut de démarrage sur le canal RTT de `defmt`.
    defmt::info!(
        "Wi-Fi AP {} started at {}. Connect to the HTTP interface on port 80.",
        WIFI_SSID,
        ACCESS_POINT_IP
    );
    // Garde la tâche principale vivante : les tâches lancées par le spawner restent alors actives.
    core::future::pending::<()>().await;
}
