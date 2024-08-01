//This is the lcd-lcm1602-i2c crate, but async for emabassy-rp
//All rights and credit to the original author
//https://github.com/KuabeM/lcd-lcm1602-i2c/tree/master

use embedded_hal_1::delay::DelayNs;

/// API to write to the LCD.
pub struct Lcd<'a, T, D>
where
    T: embedded_hal_async::i2c::I2c,
    D: DelayNs,
{
    i2c: T,
    address: u8,
    rows: u8,
    delay: &'a mut D,
    backlight_state: Backlight,
    cursor_on: bool,
    cursor_blink: bool,
}

pub enum DisplayControl {
    Off = 0x00,
    CursorBlink = 0x01,
    CursosOn = 0x02,
    DisplayOn = 0x04,
}

#[derive(Copy, Clone)]
pub enum Backlight {
    _Off = 0x00,
    On = 0x08,
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum Mode {
    Cmd = 0x00,
    Data = 0x01,
    DisplayControl = 0x08,
    FunctionSet = 0x20,
}

enum Commands {
    Clear = 0x01,
    ReturnHome = 0x02,
    ShiftCursor = 16 | 4,
}

enum BitMode {
    Bit4 = 0x0 << 4,
    Bit8 = 0x1 << 4,
}

impl<'a, T, D> Lcd<'a, T, D>
where
    T: embedded_hal_async::i2c::I2c,
    D: DelayNs,
{
    /// Create new instance with only the I2C and delay instance.
    pub fn new(i2c: T, delay: &'a mut D) -> Self {
        Self {
            i2c,
            delay,
            backlight_state: Backlight::On,
            address: 0,
            rows: 0,
            cursor_blink: false,
            cursor_on: false,
        }
    }

    /// Zero based number of rows.
    pub fn rows(mut self, rows: u8) -> Self {
        self.rows = rows;
        self
    }

    /// Set I2C address, see [lcd address].
    ///
    /// [lcd address]: https://badboi.dev/rust,/microcontrollers/2020/11/09/i2c-hello-world.html
    pub fn address(mut self, address: u8) -> Self {
        self.address = address;
        self
    }

    pub fn cursor_on(mut self, on: bool) -> Self {
        self.cursor_on = on;
        self
    }

    /// Initializes the hardware.
    ///
    /// Actual procedure is a bit obscure. This one was compiled from this [blog post],
    /// corresponding [code] and the [datasheet].
    ///
    /// [datasheet]: https://www.openhacks.com/uploadsproductos/eone-1602a1.pdf
    /// [code]: https://github.com/jalhadi/i2c-hello-world/blob/main/src/main.rs
    /// [blog post]: https://badboi.dev/rust,/microcontrollers/2020/11/09/i2c-hello-world.html
    pub async fn init(mut self) -> Result<Self, T::Error> {
        // Initial delay to wait for init after power on.
        self.delay.delay_ms(80);

        // Init with 8 bit mode
        let mode_8bit = Mode::FunctionSet as u8 | BitMode::Bit8 as u8;
        self.write4bits(mode_8bit).await?;
        self.delay.delay_ms(5);
        self.write4bits(mode_8bit).await?;
        self.delay.delay_ms(5);
        self.write4bits(mode_8bit).await?;
        self.delay.delay_ms(5);

        // Switch to 4 bit mode
        let mode_4bit = Mode::FunctionSet as u8 | BitMode::Bit4 as u8;
        self.write4bits(mode_4bit).await?;

        // Function set command
        let lines = if self.rows == 0 { 0x00 } else { 0x08 };
        self.command(
            Mode::FunctionSet as u8 |
            // 5x8 display: 0x00, 5x10: 0x4
            lines, // Two line display
        )
        .await?;

        let display_ctrl = if self.cursor_on {
            DisplayControl::DisplayOn as u8 | DisplayControl::CursosOn as u8
        } else {
            DisplayControl::DisplayOn as u8
        };
        let display_ctrl = if self.cursor_blink {
            display_ctrl | DisplayControl::CursorBlink as u8
        } else {
            display_ctrl
        };
        self.command(Mode::DisplayControl as u8 | display_ctrl)
            .await?;
        self.command(Mode::Cmd as u8 | Commands::Clear as u8)
            .await?; // Clear Display

        // Entry right: shifting cursor moves to right
        self.command(0x04).await?;
        self.backlight(self.backlight_state).await?;
        Ok(self)
    }

    async fn write4bits(&mut self, data: u8) -> Result<(), T::Error> {
        self.i2c
            .write(
                self.address,
                &[data | DisplayControl::DisplayOn as u8 | self.backlight_state as u8],
            )
            .await?;

        self.i2c
            .write(
                self.address,
                &[DisplayControl::Off as u8 | self.backlight_state as u8],
            )
            .await?;
        self.delay.delay_us(700);
        Ok(())
    }

    async fn send(&mut self, data: u8, mode: Mode) -> Result<(), T::Error> {
        let high_bits: u8 = data & 0xf0;
        let low_bits: u8 = (data << 4) & 0xf0;
        self.write4bits(high_bits | mode as u8).await?;
        self.write4bits(low_bits | mode as u8).await?;
        Ok(())
    }

    async fn command(&mut self, data: u8) -> Result<(), T::Error> {
        self.send(data, Mode::Cmd).await
    }

    pub async fn backlight(&mut self, backlight: Backlight) -> Result<(), T::Error> {
        self.backlight_state = backlight;
        self.i2c
            .write(
                self.address,
                &[DisplayControl::DisplayOn as u8 | backlight as u8],
            )
            .await
    }

    /// Write string to display.
    pub async fn write_str(&mut self, data: &str) -> Result<(), T::Error> {
        for c in data.chars() {
            self.send(c as u8, Mode::Data).await?;
        }
        Ok(())
    }

    /// Clear the display
    pub async fn clear(&mut self) -> Result<(), T::Error> {
        self.command(Commands::Clear as u8).await?;
        self.delay.delay_ms(2);
        Ok(())
    }

    /// Return cursor to upper left corner, i.e. (0,0).
    pub async fn return_home(&mut self) -> Result<(), T::Error> {
        self.command(Commands::ReturnHome as u8).await?;
        self.delay.delay_ms(2);
        Ok(())
    }

    /// Set the cursor to (rows, col). Coordinates are zero-based.
    pub async fn set_cursor(&mut self, row: u8, col: u8) -> Result<(), T::Error> {
        self.return_home().await?;
        let shift: u8 = row * 40 + col;
        for _i in 0..shift {
            self.command(Commands::ShiftCursor as u8).await?;
        }
        Ok(())
    }
}
