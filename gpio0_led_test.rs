#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};
use panic_probe as _;

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_0, Level::Low);

    loop {
        led.set_high();
        Timer::after(Duration::from_secs(1)).await;
        led.set_low();
        Timer::after(Duration::from_secs(1)).await;
    }
}
