use libm::{atan2f, fabsf, sqrtf};

enum Addresses {
    Addr = 0x53,
    PowerCtl = 0x2D,
    DataFormat = 0x31,
    DATAX0 = 0x32,
}

pub struct Adxl345<I2C: embedded_hal_async::i2c::I2c> {
    bus: I2C,
}

impl<I2C: embedded_hal_async::i2c::I2c> Adxl345<I2C> {
    pub async fn new(bus: I2C) -> Self {
        let mut sensor = Adxl345 { bus };
        sensor.init().await;
        sensor
    }

    async fn init(&mut self) {
        let _ = self
            .bus
            .write(Addresses::Addr as u8, &[Addresses::PowerCtl as u8, 0x08])
            .await;
        let _ = self
            .bus
            .write(Addresses::Addr as u8, &[Addresses::DataFormat as u8, 0x0B])
            .await;
    }

    pub async fn read_acceleration(&mut self) -> Result<(i16, i16, i16), I2C::Error> {
        let mut data = [0u8; 6];

        let result = self
            .bus
            .write_read(Addresses::Addr as u8, &[Addresses::DATAX0 as u8], &mut data)
            .await;
        match result {
            Ok(_) => {
                let values = self.unpack_i16x(&data).unwrap();
                Ok(values)
            }
            Err(e) => {
                // info!("error: {:?}", e);
                Err(e)
            }
        }
    }

    fn unpack_i16x(&mut self, data: &[u8]) -> Result<(i16, i16, i16), &'static str> {
        if data.len() != 6 {
            return Err("Data length must be 6 bytes");
        }

        let x = i16::from_le_bytes(data[0..2].try_into().unwrap());
        let y = i16::from_le_bytes(data[2..4].try_into().unwrap());
        let z = i16::from_le_bytes(data[4..6].try_into().unwrap());

        Ok((x, y, z))
    }

    // Calculate the magnitude of acceleration
    pub fn calc_accel_magnitude(&mut self, x: i16, y: i16, z: i16) -> f32 {
        let x = x as f32;
        let y = y as f32;
        let z = z as f32;
        sqrtf(x * x + y * y + z * z)
    }

    // Calculate roll angle in degrees
    pub fn calc_roll(&mut self, x: i16, y: i16, z: i16) -> f32 {
        let x = x as f32;
        let y = y as f32;
        let z = z as f32;
        atan2f(y, sqrtf(x * x + z * z)) * (180.0 / core::f32::consts::PI)
    }

    // Calculate pitch angle in degrees
    pub fn calc_pitch(&mut self, x: i16, y: i16, z: i16) -> f32 {
        let x = x as f32;
        let y = y as f32;
        let z = z as f32;
        atan2f(-x, sqrtf(y * y + z * z)) * (180.0 / core::f32::consts::PI)
    }

    pub fn detect_step(
        &mut self,
        current_magnitude: f32,
        prev_magnitude: f32,
        threshold: f32,
    ) -> bool {
        if fabsf(current_magnitude - prev_magnitude) > fabsf(threshold) {
            return true;
        }
        false
    }
}
