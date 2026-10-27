#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config, UsbDevice};
use panic_probe as _;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type UsbDriver = Driver<'static, USB>;
type UsbDeviceType = UsbDevice<'static, UsbDriver>;

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDeviceType) -> ! {
    usb.run().await
}

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());
    let driver = Driver::new(peripherals.USB, Irqs);

    let config = {
        let mut config = Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("PastoRust");
        config.product = Some("USB Hello test");
        config.serial_number = Some("PDO60-26001");
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config
    };

    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();

        Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [],
            CONTROL_BUFFER.init([0; 64]),
        )
    };

    let mut serial = {
        static SERIAL_STATE: StaticCell<State> = StaticCell::new();
        CdcAcmClass::new(&mut builder, SERIAL_STATE.init(State::new()), 64)
    };

    let usb = builder.build();
    spawner.spawn(usb_task(usb).expect("failed to create USB device task"));

    loop {
        serial.wait_connection().await;
        loop {
            if serial.write_packet(b"Hello world\r\n").await.is_err() {
                break;
            }
            Timer::after(Duration::from_secs(5)).await;
        }
    }
}
