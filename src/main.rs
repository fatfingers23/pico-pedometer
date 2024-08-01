#![no_std]
#![no_main]

mod lcd_lcm1602_i2c;

use adxl345::{DATA_FORMAT, POWER_CTL};
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::i2c::{self, Async, Config, Error, Instance, InterruptHandler};
use embassy_time::{Delay, Duration, Timer};

use lcd_lcm1602_i2c::{Backlight, DisplayControl};
use libm::{atan2f, fabsf, sqrtf};

use {defmt_rtt as _, panic_probe as _};

#[allow(dead_code)]
mod lcd {
    pub const ADDR: u8 = 0x27;
}

#[allow(dead_code)]
mod adxl345 {
    pub const ADDR: u8 = 0x53;
    pub const POWER_CTL: u8 = 0x2D;
    pub const DATA_FORMAT: u8 = 0x31;
    pub const DATAX0: u8 = 0x32;
}

embassy_rp::bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<embassy_rp::peripherals::I2C0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    use adxl345::*;
    use lcd::*;

    let p = embassy_rp::init(Default::default());
    info!("setting up i2c ");
    let sda = p.PIN_20;
    let scl = p.PIN_21;

    let mut config = Config::default();
    config.frequency = 50_000;
    let mut i2c0_bus: &mut embassy_rp::i2c::I2c<
        embassy_rp::peripherals::I2C0,
        embassy_rp::i2c::Async,
    > = &mut embassy_rp::i2c::I2c::new_async(p.I2C0, scl, sda, Irqs, config);
    // let mut i2c = i2c::I2c::new_blocking(p.I2C0, scl, sda, Config::default());

    // let mut accelerometer = Adxl345::new(i2c0_bus).await;
    info!("Done setting up i2c and sensors");

    let mut step_count = 0;
    let mut prev_magnitude = 0.0;
    let threshold = 25.0; // Adjust this threshold based on your needs

    let mut delay = Delay;
    let lcd_builder = lcd_lcm1602_i2c::Lcd::new(&mut i2c0_bus, &mut delay)
        .address(lcd::ADDR)
        .cursor_on(true) // no visible cursos
        .rows(2); // two rows

    let mut lcd = lcd_builder.init().await.unwrap();
    lcd.clear().await;
    // lcd.return_home().await;
    // lcd.clear().unwrap();
    // i2c0_bus.write_async(
    //     lcd::ADDR,
    //     &[1 | DisplayControl::DisplayOn as u8 | 0x08 as u8],
    // );

    // lcd.return_home();
    let result = lcd.write_str("No this didn't").await;
    lcd.set_cursor(1, 0).await;
    let result = lcd.write_str("take 6 hours").await;
    if let Err(e) = result {
        info!("error: {:?}", e);
    }
    // lcd.backlight(Backlight::Off).unwrap();
    loop {
        let mut data = [0u8; 6];
        // for i in 0..=127u8 {
        //     let mut readbuf: [u8; 1] = [0; 1];
        //     let result = i2c0_bus.read(i, &mut readbuf);
        //     if let Ok(d) = result {
        //         // Do whatever work you want to do with found devices
        //         // writeln!(uart, "Device found at address{:?}", i).unwrap();
        //         //hex of i
        //         info!("Device found at address {0:02X}", i);
        //     }
        // }

        //Reading the accelerometer data
        // let result = accelerometer.read_acceleration().await;
        // match result {
        //     Ok(result) => {
        //         let (x, y, z) = result;
        //         let accel_magnitude = calc_accel_magnitude(x, y, z);
        //         let roll = calc_roll(x, y, z);
        //         let pitch = calc_pitch(x, y, z);
        //         info!(
        //             "x: {}, y: {}, z: {}, accel: {}, roll: {}, pitch: {}",
        //             x, y, z, accel_magnitude, roll, pitch
        //         );

        //         if detect_step(accel_magnitude, prev_magnitude, threshold) {
        //             step_count += 1;
        //             defmt::info!("Step detected! Total steps: {}", step_count);
        //         }

        //         prev_magnitude = accel_magnitude;
        //     }
        //     Err(e) => {
        //         info!("error: {:?}", e);
        //         // Timer::after_secs(1).await;
        //         continue;
        //     }
        // }

        Timer::after_secs(1).await;
    }
}

fn unpack_i16x3(data: &[u8]) -> Result<(i16, i16, i16), &'static str> {
    if data.len() != 6 {
        return Err("Data length must be 6 bytes");
    }

    let x = i16::from_le_bytes(data[0..2].try_into().unwrap());
    let y = i16::from_le_bytes(data[2..4].try_into().unwrap());
    let z = i16::from_le_bytes(data[4..6].try_into().unwrap());

    Ok((x, y, z))
}

// Calculate the magnitude of acceleration
fn calc_accel_magnitude(x: i16, y: i16, z: i16) -> f32 {
    let x = x as f32;
    let y = y as f32;
    let z = z as f32;
    sqrtf(x * x + y * y + z * z)
}

// Calculate roll angle in degrees
fn calc_roll(x: i16, y: i16, z: i16) -> f32 {
    let x = x as f32;
    let y = y as f32;
    let z = z as f32;
    atan2f(y, sqrtf(x * x + z * z)) * (180.0 / core::f32::consts::PI)
}

// Calculate pitch angle in degrees
fn calc_pitch(x: i16, y: i16, z: i16) -> f32 {
    let x = x as f32;
    let y = y as f32;
    let z = z as f32;
    atan2f(-x, sqrtf(y * y + z * z)) * (180.0 / core::f32::consts::PI)
}

fn detect_step(current_magnitude: f32, prev_magnitude: f32, threshold: f32) -> bool {
    if fabsf(current_magnitude - prev_magnitude) > fabsf(threshold) {
        return true;
    }
    false
}

struct Adxl345<'d, T: embassy_rp::i2c::Instance, Async: embassy_rp::i2c::Mode> {
    bus: embassy_rp::i2c::I2c<'d, T, Async>,
}

impl<'d, T: Instance> Adxl345<'d, T, Async> {
    pub async fn new(bus: embassy_rp::i2c::I2c<'d, T, Async>) -> Self {
        let mut sensor = Adxl345 { bus };
        sensor.init().await;
        sensor
    }

    async fn init(&mut self) {
        let _ = self.bus.write_async(adxl345::ADDR, [POWER_CTL, 0x08]).await;
        let _ = self
            .bus
            .write_async(adxl345::ADDR, [DATA_FORMAT, 0x0B])
            .await;
    }

    pub async fn read_acceleration(&mut self) -> Result<(i16, i16, i16), Error> {
        let mut data = [0u8; 6];

        let result = self
            .bus
            .write_read_async(adxl345::ADDR, [adxl345::DATAX0], &mut data)
            .await;
        match result {
            Ok(_) => {
                let values = unpack_i16x3(&data).unwrap();
                Ok(values)
            }
            Err(e) => {
                info!("error: {:?}", e);
                Err(e)
            }
        }
    }
}
