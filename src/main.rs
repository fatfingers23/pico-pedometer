#![no_std]
#![no_main]

mod adxl345;
mod lcd_lcm1602_i2c;

use adxl345::Adxl345;
use core::sync::atomic::AtomicU32;
use defmt::*;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::i2c::Config;
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Timer};
use heapless::String;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

type I2c1Bus = Mutex<NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;

embassy_rp::bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

static STEPS: AtomicU32 = AtomicU32::new(0);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("setting up i2c ");
    let sda = p.PIN_20;
    let scl = p.PIN_21;

    let mut config = Config::default();
    config.frequency = 50_000;
    let i2c = I2c::new_async(p.I2C0, scl, sda, Irqs, config);
    static I2C_BUS: StaticCell<I2c1Bus> = StaticCell::new();
    let i2c_bus = I2C_BUS.init(Mutex::new(i2c));

    spawner.must_spawn(lcd_task(i2c_bus));
    spawner.must_spawn(pedometer_task(i2c_bus));
}

#[embassy_executor::task]
async fn lcd_task(i2c_bus: &'static I2c1Bus) {
    let i2c_dev = I2cDevice::new(i2c_bus);

    let mut delay = Delay;
    let lcd_builder = lcd_lcm1602_i2c::Lcd::new(i2c_dev, &mut delay)
        .address(0x27)
        .cursor_on(false) // no visible cursos
        .rows(2); // two rows

    let mut lcd = lcd_builder.init().await.unwrap();
    let _ = lcd.clear().await;

    loop {
        let steps = STEPS.load(core::sync::atomic::Ordering::Relaxed);
        //steps bytes
        let mut steps_str = String::<16>::new();
        core::fmt::write(&mut steps_str, format_args!("{}", steps)).unwrap();
        let _ = lcd.set_cursor(0, 0).await;
        let _ = lcd.write_str("Steps: ").await;
        let _ = lcd.write_str(steps_str.as_str()).await;
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn pedometer_task(i2c_bus: &'static I2c1Bus) {
    let i2c_dev = I2cDevice::new(i2c_bus);

    let mut accelerometer = Adxl345::new(i2c_dev).await;

    let mut step_count = 0;
    let mut prev_magnitude = 0.0;
    let threshold = 10.0; // Adjust this threshold based on your need

    loop {
        //Reading the accelerometer data
        let result = accelerometer.read_acceleration().await;
        match result {
            Ok(result) => {
                let (x, y, z) = result;

                let accel_magnitude = accelerometer.calc_accel_magnitude(x, y, z);
                let roll = accelerometer.calc_roll(x, y, z);
                let pitch = accelerometer.calc_pitch(x, y, z);
                info!(
                    "x: {}, y: {}, z: {}, accel: {}, roll: {}, pitch: {}",
                    x, y, z, accel_magnitude, roll, pitch
                );

                if accelerometer.detect_step(accel_magnitude, prev_magnitude, threshold) {
                    step_count += 1;
                    info!("Step detected! Total steps: {}", step_count);
                    STEPS.store(step_count, core::sync::atomic::Ordering::Relaxed);
                }

                prev_magnitude = accel_magnitude;
            }
            Err(e) => {
                info!("error: {:?}", e);
                // Timer::after_secs(1).await;
                continue;
            }
        }

        Timer::after_millis(500).await;
    }
}
